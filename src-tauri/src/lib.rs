//! FlowSTT GUI - Tauri application with integrated engine.
//!
//! The engine (audio capture, transcription, IPC server) runs in-process.
//! Tauri commands call engine functions directly without IPC serialization.
//! The IPC socket server is hosted by this process for CLI client access.

mod tray;

use flowstt_common::config::{Config, ThemeMode};
use flowstt_common::ipc::{EventType, Request, Response};
use flowstt_common::{runtime_mode, AudioDevice, HotkeyCombination, RecordingMode, RuntimeMode, TranscriptionMode};
use std::env;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tauri::webview::WebviewWindowBuilder;
use tauri::WebviewUrl;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

/// Detect if running on Wayland and set workaround env vars (Linux-specific)
#[cfg(target_os = "linux")]
fn configure_wayland_workarounds() {
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false);

    if is_wayland {
        // SAFETY: This is called before any threads are spawned
        unsafe {
            env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_wayland_workarounds() {}

/// Initialize logging based on runtime mode.
fn init_logging() {
    let mode = runtime_mode();

    match mode {
        RuntimeMode::Production => {
            if let Err(e) = flowstt_common::logging::ensure_log_dir() {
                eprintln!("Warning: Failed to create log directory, using temp dir: {}", e);
            }

            let log_path = flowstt_common::logging::app_log_path();
            let log_dir = log_path.parent().unwrap();

            let file_appender = match tracing_appender::rolling::RollingFileAppender::builder()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .max_log_files(5)
                .filename_prefix("flowstt-app")
                .filename_suffix("log")
                .build(log_dir)
            {
                Ok(appender) => appender,
                Err(e) => {
                    eprintln!("Warning: Failed to create log file appender: {}", e);
                    let temp_dir = std::env::temp_dir().join("flowstt-logs");
                    let _ = std::fs::create_dir_all(&temp_dir);
                    tracing_appender::rolling::RollingFileAppender::builder()
                        .rotation(tracing_appender::rolling::Rotation::DAILY)
                        .max_log_files(5)
                        .filename_prefix("flowstt-app")
                        .filename_suffix("log")
                        .build(&temp_dir)
                        .expect("Failed to create temp log file appender")
                }
            };

            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            std::mem::forget(guard);

            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .with_writer(non_blocking)
                .with_ansi(false)
                .init();

            info!("Production logging initialized");
        }
        RuntimeMode::Development => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
                )
                .init();

            debug!("Development logging initialized (console only)");
        }
    }
}

// ─── Application state ───────────────────────────────────────────────────────

/// Application state shared between Tauri commands.
struct AppState {
    /// Handle to the IPC server task
    ipc_server_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

// ─── Event callback for Tauri frontend ───────────────────────────────────────

/// Forwards engine events directly to the Tauri frontend via emit().
struct TauriEventCallback {
    app_handle: AppHandle,
}

impl flowstt_engine::ipc::EventCallback for TauriEventCallback {
    fn on_event(&self, event: &EventType) {
        forward_event_to_tauri(&self.app_handle, event);
    }
}

/// Forward an engine event to the Tauri frontend.
fn forward_event_to_tauri(app_handle: &AppHandle, event: &EventType) {
    match event {
        EventType::VisualizationData(data) => {
            let _ = app_handle.emit("visualization-data", data);
        }
        EventType::TranscriptionComplete(result) => {
            let _ = app_handle.emit("transcription-complete", result);

            // On Windows, WebView2 can enter a frozen rendering state when
            // Alt (the default PTT key) is released while the window is focused.
            #[cfg(target_os = "windows")]
            {
                for label in ["main", "setup"] {
                    if let Some(win) = app_handle.get_webview_window(label) {
                        if let Ok(hwnd) = win.hwnd() {
                            unsafe {
                                let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                                    windows::Win32::Foundation::HWND(hwnd.0),
                                    windows::Win32::UI::WindowsAndMessaging::WM_CANCELMODE,
                                    None,
                                    None,
                                );
                            }
                        }
                    }
                }
            }
        }
        EventType::SpeechStarted => {
            let _ = app_handle.emit("speech-started", ());
        }
        EventType::SpeechEnded { duration_ms } => {
            let _ = app_handle.emit("speech-ended", duration_ms);
        }
        EventType::CaptureStateChanged { capturing, error } => {
            #[derive(serde::Serialize, Clone)]
            struct CaptureState {
                capturing: bool,
                error: Option<String>,
            }
            let _ = app_handle.emit(
                "capture-state-changed",
                CaptureState {
                    capturing: *capturing,
                    error: error.clone(),
                },
            );
        }
        EventType::ModelDownloadProgress { percent } => {
            let _ = app_handle.emit("model-download-progress", percent);
        }
        EventType::ModelDownloadComplete { success } => {
            let _ = app_handle.emit("model-download-complete", success);
        }
        EventType::AudioLevelUpdate {
            device_id,
            level_db,
        } => {
            #[derive(serde::Serialize, Clone)]
            struct AudioLevel {
                device_id: String,
                level_db: f32,
            }
            let _ = app_handle.emit(
                "audio-level-update",
                AudioLevel {
                    device_id: device_id.clone(),
                    level_db: *level_db,
                },
            );
        }
        EventType::PttPressed => {
            let _ = app_handle.emit("ptt-pressed", ());
        }
        EventType::PttReleased => {
            let _ = app_handle.emit("ptt-released", ());
        }
        EventType::TranscriptionModeChanged { mode } => {
            let _ = app_handle.emit("transcription-mode-changed", mode);
        }
        EventType::AutoModeToggled { mode } => {
            let _ = app_handle.emit("auto-mode-toggled", mode);
        }
        EventType::HistoryEntryDeleted { id } => {
            let _ = app_handle.emit("history-entry-deleted", id);
        }
        EventType::Shutdown => {
            let _ = app_handle.emit("service-shutdown", ());
        }
    }
}

// ─── Tauri commands (call engine directly) ───────────────────────────────────

/// List all available audio sources
#[tauri::command]
async fn list_all_sources() -> Result<Vec<AudioDevice>, String> {
    let response = flowstt_engine::ipc::handlers::handle_request(
        Request::ListDevices { source_type: None },
    )
    .await;
    match response {
        Response::Devices { devices } => Ok(devices),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set audio sources
#[tauri::command]
async fn set_sources(
    source1_id: Option<String>,
    source2_id: Option<String>,
) -> Result<(), String> {
    let response = flowstt_engine::ipc::handlers::handle_request(Request::SetSources {
        source1_id,
        source2_id,
    })
    .await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set echo cancellation enabled/disabled
#[tauri::command]
async fn set_aec_enabled(enabled: bool) -> Result<(), String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::SetAecEnabled { enabled }).await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set recording mode
#[tauri::command]
async fn set_recording_mode(mode: RecordingMode) -> Result<(), String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::SetRecordingMode { mode }).await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Local model status struct for frontend compatibility
#[derive(serde::Serialize)]
struct LocalModelStatus {
    available: bool,
    path: String,
}

/// Check Whisper model status
#[tauri::command]
async fn check_model_status() -> Result<LocalModelStatus, String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::GetModelStatus).await;
    match response {
        Response::ModelStatus(status) => Ok(LocalModelStatus {
            available: status.available,
            path: status.path,
        }),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Download the Whisper model
#[tauri::command]
async fn download_model() -> Result<(), String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::DownloadModel).await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Local CUDA status struct for frontend compatibility
#[derive(serde::Serialize)]
struct LocalCudaStatus {
    build_enabled: bool,
    runtime_available: bool,
    system_info: String,
}

/// Get CUDA/GPU acceleration status
#[tauri::command]
async fn get_cuda_status() -> Result<LocalCudaStatus, String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::GetCudaStatus).await;
    match response {
        Response::CudaStatus(status) => Ok(LocalCudaStatus {
            build_enabled: status.build_enabled,
            runtime_available: status.runtime_available,
            system_info: status.system_info,
        }),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Status struct for frontend
#[derive(serde::Serialize)]
struct LocalStatus {
    capturing: bool,
    in_speech: bool,
    queue_depth: usize,
    error: Option<String>,
    source1_id: Option<String>,
    source2_id: Option<String>,
    transcription_mode: TranscriptionMode,
}

/// Get current status
#[tauri::command]
async fn get_status() -> Result<LocalStatus, String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::GetStatus).await;
    match response {
        Response::Status(status) => Ok(LocalStatus {
            capturing: status.capturing,
            in_speech: status.in_speech,
            queue_depth: status.queue_depth,
            error: status.error,
            source1_id: status.source1_id,
            source2_id: status.source2_id,
            transcription_mode: status.transcription_mode,
        }),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Push-to-talk status for frontend
#[derive(serde::Serialize)]
struct LocalPttStatus {
    mode: TranscriptionMode,
    hotkeys: Vec<HotkeyCombination>,
    auto_toggle_hotkeys: Vec<HotkeyCombination>,
    auto_mode_active: bool,
    is_active: bool,
    available: bool,
    error: Option<String>,
}

/// Set the transcription mode
#[tauri::command]
async fn set_transcription_mode(mode: TranscriptionMode) -> Result<(), String> {
    let response = flowstt_engine::ipc::handlers::handle_request(
        Request::SetTranscriptionMode { mode },
    )
    .await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set the push-to-talk hotkey combinations
#[tauri::command]
async fn set_ptt_hotkeys(hotkeys: Vec<HotkeyCombination>) -> Result<(), String> {
    let response = flowstt_engine::ipc::handlers::handle_request(
        Request::SetPushToTalkHotkeys { hotkeys },
    )
    .await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Get push-to-talk status
#[tauri::command]
async fn get_ptt_status() -> Result<LocalPttStatus, String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::GetPttStatus).await;
    match response {
        Response::PttStatus(status) => Ok(LocalPttStatus {
            mode: status.mode,
            hotkeys: status.hotkeys,
            auto_toggle_hotkeys: status.auto_toggle_hotkeys,
            auto_mode_active: status.auto_mode_active,
            is_active: status.is_active,
            available: status.available,
            error: status.error,
        }),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set the auto-mode toggle hotkeys
#[tauri::command]
async fn set_auto_toggle_hotkeys(hotkeys: Vec<HotkeyCombination>) -> Result<(), String> {
    let response = flowstt_engine::ipc::handlers::handle_request(
        Request::SetAutoToggleHotkeys { hotkeys },
    )
    .await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Toggle between Automatic and PushToTalk modes
#[tauri::command]
async fn toggle_auto_mode() -> Result<TranscriptionMode, String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::ToggleAutoMode).await;
    match response {
        Response::Ok => {
            let status_response =
                flowstt_engine::ipc::handlers::handle_request(Request::GetPttStatus).await;
            match status_response {
                Response::PttStatus(status) => Ok(status.mode),
                Response::Error { message } => Err(message),
                _ => Err("Unexpected response".into()),
            }
        }
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// History entry struct for frontend compatibility
#[derive(serde::Serialize, serde::Deserialize)]
struct LocalHistoryEntry {
    id: String,
    text: String,
    timestamp: String,
    wav_path: Option<String>,
}

/// Get transcription history
#[tauri::command]
async fn get_history() -> Result<Vec<LocalHistoryEntry>, String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::GetHistory).await;
    match response {
        Response::History { entries } => Ok(entries
            .into_iter()
            .map(|e| LocalHistoryEntry {
                id: e.id,
                text: e.text,
                timestamp: e.timestamp,
                wav_path: e.wav_path,
            })
            .collect()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Delete a history entry
#[tauri::command]
async fn delete_history_entry(id: String) -> Result<(), String> {
    let response = flowstt_engine::ipc::handlers::handle_request(
        Request::DeleteHistoryEntry { id },
    )
    .await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Get the current theme mode from the config file.
#[tauri::command]
fn get_theme_mode() -> Result<ThemeMode, String> {
    let config = Config::load();
    Ok(config.theme_mode)
}

/// Set the theme mode and persist to the config file.
#[tauri::command]
fn set_theme_mode(mode: ThemeMode, app_handle: AppHandle) -> Result<(), String> {
    let mut config = Config::load();
    config.theme_mode = mode.clone();
    config.save().map_err(|e| format!("Failed to save config: {}", e))?;
    app_handle
        .emit("theme-changed", &mode)
        .map_err(|e| format!("Failed to emit theme event: {}", e))?;
    Ok(())
}

/// Check if first-time setup is needed.
#[tauri::command]
fn needs_setup() -> bool {
    Config::needs_setup()
}

/// Get the current runtime mode.
#[tauri::command]
fn get_runtime_mode() -> String {
    runtime_mode().as_str().to_string()
}

/// Cancel any pending Win32 menu activation mode on a window.
#[tauri::command]
fn cancel_menu_mode(_window: tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = _window.hwnd() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                    windows::Win32::Foundation::HWND(hwnd.0),
                    windows::Win32::UI::WindowsAndMessaging::WM_CANCELMODE,
                    None,
                    None,
                );
            }
        }
    }
}

/// Complete the first-time setup wizard.
#[tauri::command]
async fn complete_setup(
    transcription_mode: TranscriptionMode,
    hotkeys: Vec<HotkeyCombination>,
    source1_id: Option<String>,
    source2_id: Option<String>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let config = Config {
        transcription_mode,
        ptt_hotkeys: hotkeys,
        ..Config::default_with_hotkeys()
    };
    config
        .save()
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // Configure engine with chosen sources (direct call, no IPC)
    if source1_id.is_some() || source2_id.is_some() {
        let _ = flowstt_engine::ipc::handlers::handle_request(Request::SetSources {
            source1_id,
            source2_id,
        })
        .await;
    }

    // Set transcription mode on engine
    let _ = flowstt_engine::ipc::handlers::handle_request(
        Request::SetTranscriptionMode {
            mode: transcription_mode,
        },
    )
    .await;

    app_handle
        .emit("setup-complete", ())
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Start a test audio capture on a device for level metering.
#[tauri::command]
async fn test_audio_device(device_id: String) -> Result<(), String> {
    let response = flowstt_engine::ipc::handlers::handle_request(
        Request::TestAudioDevice { device_id },
    )
    .await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Stop any active test audio capture.
#[tauri::command]
async fn stop_test_audio_device() -> Result<(), String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::StopTestAudioDevice).await;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Open System Settings to the Accessibility pane on macOS.
/// The Tauri app process itself now needs Accessibility permission (no more IPC delegation).
#[tauri::command]
async fn open_accessibility_settings() -> Result<(), String> {
    // AXIsProcessTrustedWithOptions(prompt=true) shows the system dialog and adds the app
    // to the Accessibility list in System Settings automatically - no need to open
    // System Settings separately.
    flowstt_engine::hotkey::request_accessibility_permission();
    Ok(())
}

/// Check whether this process has macOS Accessibility permission.
/// Now checks directly (no IPC to separate service).
#[tauri::command]
async fn check_accessibility_permission() -> Result<bool, String> {
    Ok(flowstt_engine::hotkey::check_accessibility_permission())
}

/// Connect events -- no-op now since events are forwarded directly from the engine
/// via the registered EventCallback. Kept for frontend compatibility.
#[tauri::command]
async fn connect_events() -> Result<(), String> {
    // Events are now forwarded directly from the engine via TauriEventCallback.
    // This command exists for frontend compatibility (the JS still calls it on startup).
    debug!("[Startup] connect_events: no-op (events forwarded directly from engine)");
    Ok(())
}

/// Log a startup diagnostic message from the frontend.
#[tauri::command]
fn startup_log(message: String) {
    info!("[Startup/JS] {}", message);
}

/// Log a message from the frontend to the log file.
#[tauri::command]
fn log_to_file(level: String, message: String) {
    match level.as_str() {
        "error" => error!("{}", message),
        "warn" => warn!("{}", message),
        "info" => info!("{}", message),
        "debug" => debug!("{}", message),
        _ => info!("{}", message),
    }
}

// ─── Application entry point ─────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_t0 = Instant::now();

    // Parse --headless flag
    let headless = std::env::args().any(|arg| arg == "--headless");

    // Initialize logging (single subscriber for both engine and GUI)
    init_logging();

    info!("[Startup] run() entered (headless={})", headless);
    configure_wayland_workarounds();

    tauri::Builder::default()
        .manage(AppState {
            ipc_server_handle: Mutex::new(None),
        })
        .setup(move |app| {
            debug!(
                "[Startup] setup() hook called (+{}ms from run())",
                app_t0.elapsed().as_millis()
            );

            #[cfg(windows)]
            if let Ok(resource_dir) = app.path().resource_dir() {
                env::set_var("FLOWSTT_RESOURCE_DIR", resource_dir);
            }

            let app_handle = app.handle().clone();

            // Register event callback so engine events go directly to Tauri frontend
            flowstt_engine::ipc::register_event_callback(TauriEventCallback {
                app_handle: app_handle.clone(),
            });

            // Initialize the engine (audio backends, transcription, IPC server, etc.)
            let ipc_handle = tauri::async_runtime::block_on(async {
                flowstt_engine::init().await
            });

            match ipc_handle {
                Ok(handle) => {
                    let state: State<AppState> = app.state();
                    let mut lock = tauri::async_runtime::block_on(state.ipc_server_handle.lock());
                    *lock = Some(handle);
                    info!("[Startup] Engine initialized successfully");
                }
                Err(e) => {
                    error!("[Startup] Failed to initialize engine: {}", e);
                }
            }

            // Set up the system tray (always, including headless mode)
            if let Err(e) = tray::setup_tray(app) {
                warn!("[FlowSTT] Failed to set up system tray: {}", e);
            }

            // First-run detection: show setup wizard if no config exists
            if Config::needs_setup() && !headless {
                info!("[Startup] First run detected - showing setup wizard");

                if let Some(main_win) = app.get_webview_window("main") {
                    let _ = main_win.hide();
                }

                let _setup_win = WebviewWindowBuilder::new(
                    app,
                    "setup",
                    WebviewUrl::App("setup.html".into()),
                )
                .title("FlowSTT Setup")
                .inner_size(600.0, 500.0)
                .min_inner_size(500.0, 400.0)
                .center()
                .decorations(false)
                .transparent(true)
                .shadow(true)
                .visible(true)
                .build()
                .expect("Failed to create setup window");

                let app_handle = app.handle().clone();
                app.listen("setup-complete", move |_event| {
                    info!("[Startup] Setup complete - transitioning to main window");

                    if let Some(setup_win) = app_handle.get_webview_window("setup") {
                        let _ = setup_win.destroy();
                    }

                    if let Some(main_win) = app_handle.get_webview_window("main") {
                        let _ = main_win.show();
                        let _ = main_win.set_focus();
                    }
                });
            } else if headless {
                // Headless mode: hide the main window, tray is already set up
                info!("[Startup] Headless mode - hiding main window");
                if let Some(main_win) = app.get_webview_window("main") {
                    let _ = main_win.hide();
                }
            }

            debug!(
                "[Startup] setup() hook done (+{}ms from run())",
                app_t0.elapsed().as_millis()
            );
            Ok(())
        })
        .on_window_event(|_window, _event| {
            #[cfg(windows)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = _event {
                if _window.label() == "main" {
                    api.prevent_close();
                    let _ = _window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            startup_log,
            log_to_file,
            list_all_sources,
            set_sources,
            set_aec_enabled,
            set_recording_mode,
            check_model_status,
            download_model,
            get_status,
            get_cuda_status,
            set_transcription_mode,
            set_ptt_hotkeys,
            get_ptt_status,
            set_auto_toggle_hotkeys,
            toggle_auto_mode,
            get_history,
            delete_history_entry,
            connect_events,
            get_theme_mode,
            set_theme_mode,
            needs_setup,
            get_runtime_mode,
            cancel_menu_mode,
            complete_setup,
            test_audio_device,
            stop_test_audio_device,
            check_accessibility_permission,
            open_accessibility_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Cleanup engine on exit
    flowstt_engine::cleanup();
    info!("FlowSTT stopped");
}
