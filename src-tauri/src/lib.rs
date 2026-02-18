//! FlowSTT GUI - Tauri application that communicates with the background service.
//!
//! This module provides the Tauri commands that the frontend uses.
//! All audio capture and transcription is handled by the service via IPC.

mod binaries;
mod ipc_client;
mod tray;

use flowstt_common::config::{Config, ThemeMode};
use flowstt_common::ipc::{Request, Response};
use flowstt_common::{runtime_mode, AudioDevice, HotkeyCombination, RecordingMode, RuntimeMode, TranscriptionMode};
use ipc_client::{IpcClient, SharedIpcClient};
use std::env;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tauri::webview::WebviewWindowBuilder;
use tauri::WebviewUrl;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

/// FFI bindings for macOS Accessibility APIs (ApplicationServices framework).
#[cfg(target_os = "macos")]
mod macos_ffi {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        /// Returns true if the current process has been trusted for Accessibility access.
        pub fn AXIsProcessTrusted() -> bool;
    }
}

/// Detect if running on Wayland and set workaround env vars (Linux-specific)
#[cfg(target_os = "linux")]
fn configure_wayland_workarounds() {
    // Check for Wayland session
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false);

    if is_wayland {
        // WebKitGTK has compositing issues on Wayland
        // SAFETY: This is called before any threads are spawned
        unsafe {
            env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_wayland_workarounds() {
    // No-op on non-Linux platforms
}

/// Initialize logging based on runtime mode.
///
/// In production mode: logs to file with rotation.
/// In development mode: logs to console.
fn init_logging() {
    let mode = runtime_mode();

    match mode {
        RuntimeMode::Production => {
            // Ensure log directory exists
            if let Err(e) = flowstt_common::logging::ensure_log_dir() {
                eprintln!("Warning: Failed to create log directory, using temp dir: {}", e);
            }

            let log_path = flowstt_common::logging::app_log_path();
            let log_dir = log_path.parent().unwrap();

            // Create rolling file appender (rotates daily, max 5 files)
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
                    // Fall back to temp directory
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
            // Keep guard alive for the entire application lifetime
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

/// Application state shared between Tauri commands.
struct AppState {
    /// Shared IPC client for communication with the service
    ipc: SharedIpcClient,
    /// Handle to the event forwarding task
    event_task_running: Arc<Mutex<bool>>,
}

/// Helper to send a request to the service and handle errors.
async fn send_request(ipc: &SharedIpcClient, request: Request) -> Result<Response, String> {
    let t0 = Instant::now();
    let mut client = ipc.client.lock().await;
    let lock_ms = t0.elapsed().as_millis();
    if lock_ms > 5 {
        debug!("[Startup] send_request: waited {}ms for ipc lock", lock_ms);
    }
    client
        .request(request)
        .await
        .map_err(|e| format!("IPC error: {}", e))
}

/// List all available audio sources (both input devices and system audio monitors)
#[tauri::command]
async fn list_all_sources(state: State<'_, AppState>) -> Result<Vec<AudioDevice>, String> {
    let response = send_request(&state.ipc, Request::ListDevices { source_type: None }).await?;

    match response {
        Response::Devices { devices } => Ok(devices),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set audio sources - capture starts automatically when valid sources are configured
#[tauri::command]
async fn set_sources(
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let response = send_request(
        &state.ipc,
        Request::SetSources {
            source1_id,
            source2_id,
        },
    )
    .await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set echo cancellation enabled/disabled
#[tauri::command]
async fn set_aec_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::SetAecEnabled { enabled }).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set recording mode
#[tauri::command]
async fn set_recording_mode(mode: RecordingMode, state: State<'_, AppState>) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::SetRecordingMode { mode }).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Check Whisper model status
#[tauri::command]
async fn check_model_status(state: State<'_, AppState>) -> Result<LocalModelStatus, String> {
    let response = send_request(&state.ipc, Request::GetModelStatus).await?;

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
async fn download_model(state: State<'_, AppState>) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::DownloadModel).await?;

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

/// Local CUDA status struct for frontend compatibility
#[derive(serde::Serialize)]
struct LocalCudaStatus {
    build_enabled: bool,
    runtime_available: bool,
    system_info: String,
}

/// Get CUDA/GPU acceleration status
#[tauri::command]
async fn get_cuda_status(state: State<'_, AppState>) -> Result<LocalCudaStatus, String> {
    let response = send_request(&state.ipc, Request::GetCudaStatus).await?;

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
async fn get_status(state: State<'_, AppState>) -> Result<LocalStatus, String> {
    let response = send_request(&state.ipc, Request::GetStatus).await?;

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

/// Set the transcription mode (Automatic or PushToTalk)
#[tauri::command]
async fn set_transcription_mode(
    mode: TranscriptionMode,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::SetTranscriptionMode { mode }).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set the push-to-talk hotkey combinations
#[tauri::command]
async fn set_ptt_hotkeys(
    hotkeys: Vec<HotkeyCombination>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let response =
        send_request(&state.ipc, Request::SetPushToTalkHotkeys { hotkeys }).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Get push-to-talk status
#[tauri::command]
async fn get_ptt_status(state: State<'_, AppState>) -> Result<LocalPttStatus, String> {
    let response = send_request(&state.ipc, Request::GetPttStatus).await?;

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
async fn set_auto_toggle_hotkeys(
    hotkeys: Vec<HotkeyCombination>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let response =
        send_request(&state.ipc, Request::SetAutoToggleHotkeys { hotkeys }).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Toggle between Automatic and PushToTalk modes
#[tauri::command]
async fn toggle_auto_mode(state: State<'_, AppState>) -> Result<TranscriptionMode, String> {
    let response = send_request(&state.ipc, Request::ToggleAutoMode).await?;

    match response {
        Response::Ok => {
            // Get the new mode
            let status_response = send_request(&state.ipc, Request::GetPttStatus).await?;
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
async fn get_history(state: State<'_, AppState>) -> Result<Vec<LocalHistoryEntry>, String> {
    let response = send_request(&state.ipc, Request::GetHistory).await?;

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
async fn delete_history_entry(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let response =
        send_request(&state.ipc, Request::DeleteHistoryEntry { id }).await?;

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
/// Emits a "theme-changed" event to all windows so they can update.
#[tauri::command]
fn set_theme_mode(mode: ThemeMode, app_handle: AppHandle) -> Result<(), String> {
    let mut config = Config::load();
    config.theme_mode = mode.clone();
    config.save().map_err(|e| format!("Failed to save config: {}", e))?;
    // Emit event to all windows
    app_handle
        .emit("theme-changed", &mode)
        .map_err(|e| format!("Failed to emit theme event: {}", e))?;
    Ok(())
}

/// Check if first-time setup is needed (no config file exists).
#[tauri::command]
fn needs_setup() -> bool {
    Config::needs_setup()
}

/// Get the current runtime mode (development or production).
#[tauri::command]
fn get_runtime_mode() -> String {
    runtime_mode().as_str().to_string()
}

/// Cancel any pending Win32 menu activation mode on a window.
///
/// On Windows, releasing the Alt key sends WM_SYSKEYUP which triggers
/// the menu bar activation heuristic in WebView2. Even in a decorationless
/// window this suspends the compositor/rendering. Sending WM_CANCELMODE
/// to the HWND cancels this state and restores normal rendering.
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
/// Saves the chosen configuration and signals the setup window to close.
#[tauri::command]
async fn complete_setup(
    transcription_mode: TranscriptionMode,
    hotkeys: Vec<HotkeyCombination>,
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Build and save config
    let config = Config {
        transcription_mode,
        ptt_hotkeys: hotkeys,
        ..Config::default_with_hotkeys()
    };
    config
        .save()
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // Configure service with chosen sources (if any)
    if source1_id.is_some() || source2_id.is_some() {
        let _ = send_request(
            &state.ipc,
            Request::SetSources {
                source1_id,
                source2_id,
            },
        )
        .await;
    }

    // Set transcription mode on service
    let _ = send_request(
        &state.ipc,
        Request::SetTranscriptionMode {
            mode: transcription_mode,
        },
    )
    .await;

    // Emit setup-complete event to transition to main window
    app_handle
        .emit("setup-complete", ())
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Start a test audio capture on a device for level metering.
#[tauri::command]
async fn test_audio_device(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let response = send_request(
        &state.ipc,
        Request::TestAudioDevice { device_id },
    )
    .await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Stop any active test audio capture.
#[tauri::command]
async fn stop_test_audio_device(state: State<'_, AppState>) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::StopTestAudioDevice).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Notify the service that the GUI process has confirmed Accessibility permission is granted.
/// The service binary is unsigned and AXIsProcessTrusted() returns false in its own process
/// context. This signal lets it skip that check and proceed to CGEventTapCreate directly.
#[tauri::command]
async fn notify_accessibility_permission_granted(
    granted: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let response = send_request(
        &state.ipc,
        Request::SetAccessibilityPermissionGranted { granted },
    )
    .await?;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Open macOS System Settings at Privacy & Security → Accessibility.
/// No-op on non-macOS platforms.
#[tauri::command]
fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

/// Check whether the process currently has macOS Accessibility permission.
/// Returns true on non-macOS platforms (permission not applicable).
/// Used by the setup wizard to poll permission state on macOS.
#[tauri::command]
fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: AXIsProcessTrusted() is safe to call at any time.
        unsafe { macos_ffi::AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Log a startup diagnostic message from the frontend.
/// In production mode, writes to the log file. In development mode, writes to stderr.
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

/// Connect to the service and start event forwarding.
/// The service is already operational; this just subscribes to its event stream.
///
/// This eagerly connects the shared IPC client (used by all request/response
/// commands) before spawning the long-lived event stream task with its own
/// dedicated connection. Connecting them sequentially avoids a race where both
/// clients compete for the single available named-pipe instance on the server,
/// which would cause one of them to fall through to the 5-second spawn-retry
/// loop even though the service is already running.
#[tauri::command]
async fn connect_events(state: State<'_, AppState>, app_handle: AppHandle) -> Result<(), String> {
    let t0 = Instant::now();
    info!("[Startup] connect_events: begin");

    // Eagerly connect the shared request/response client first. This ensures
    // it has an established connection before the event task's connection
    // attempt consumes the next available pipe instance.
    {
        let mut client = state.ipc.client.lock().await;
        debug!(
            "[Startup] connect_events: acquired ipc lock (+{}ms)",
            t0.elapsed().as_millis()
        );
        client
            .connect_or_spawn()
            .await
            .map_err(|e| format!("IPC connection error: {}", e))?;
        debug!(
            "[Startup] connect_events: shared client connected (+{}ms)",
            t0.elapsed().as_millis()
        );
    }

    // Now start event forwarding with its own dedicated connection
    start_event_forwarding(app_handle, state.event_task_running.clone()).await;
    info!(
        "[Startup] connect_events: done (+{}ms)",
        t0.elapsed().as_millis()
    );
    Ok(())
}

/// Start the event forwarding task.
/// This subscribes to service events and forwards them to the Tauri frontend.
async fn start_event_forwarding(app_handle: AppHandle, running: Arc<Mutex<bool>>) {
    // Check if already running
    {
        let is_running = running.lock().await;
        if *is_running {
            debug!("[Startup] start_event_forwarding: already running, skipping");
            return;
        }
    }

    // Mark as running
    {
        let mut is_running = running.lock().await;
        *is_running = true;
    }

    // Spawn event forwarding task
    let running_clone = running.clone();
    tokio::spawn(async move {
        let t0 = Instant::now();
        info!("[Startup] EventForwarder task: begin");

        // Create a dedicated client for event streaming
        let mut event_client = IpcClient::new();

        if let Err(e) = event_client.connect_or_spawn().await {
            error!(
                "[Startup] EventForwarder task: connect FAILED (+{}ms): {}",
                t0.elapsed().as_millis(),
                e
            );
            let mut is_running = running_clone.lock().await;
            *is_running = false;
            return;
        }
        info!(
            "[Startup] EventForwarder task: connected (+{}ms)",
            t0.elapsed().as_millis()
        );

        // This will run until the connection is closed
        if let Err(e) = event_client.subscribe_and_forward(app_handle).await {
            warn!("[EventForwarder] Event stream ended: {}", e);
        }

        // Mark as not running
        let mut is_running = running_clone.lock().await;
        *is_running = false;
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_t0 = Instant::now();
    
    // Initialize logging based on runtime mode
    init_logging();
    
    info!("[Startup] run() entered");
    configure_wayland_workarounds();

    tauri::Builder::default()
        .manage(AppState {
            ipc: SharedIpcClient::new(),
            event_task_running: Arc::new(Mutex::new(false)),
        })
        .setup(move |app| {
            debug!(
                "[Startup] setup() hook called (+{}ms from run())",
                app_t0.elapsed().as_millis()
            );

            // Initialize service path (finds bundle binaries or dev location)
            let app_handle = app.handle().clone();
            ipc_client::init_service_path(&app_handle);

            // Log whether we're running from bundle
            if binaries::is_bundled(&app_handle) {
                debug!("[Startup] Running from installed app bundle");
            } else {
                debug!("[Startup] Running in development mode");
            }

            // Set up the system tray
            if let Err(e) = tray::setup_tray(app) {
                warn!("[FlowSTT] Failed to set up system tray: {}", e);
            }

            // First-run detection: show setup wizard if no config exists
            if Config::needs_setup() {
                info!("[Startup] First run detected - showing setup wizard");

                // Hide the main window (it starts hidden via tauri.conf.json,
                // but ensure it stays hidden)
                if let Some(main_win) = app.get_webview_window("main") {
                    let _ = main_win.hide();
                }

                // Create the setup window
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

                // Listen for setup completion
                let app_handle = app.handle().clone();
                app.listen("setup-complete", move |_event| {
                    info!("[Startup] Setup complete - transitioning to main window");

                    // Close the setup window
                    if let Some(setup_win) = app_handle.get_webview_window("setup") {
                        let _ = setup_win.destroy();
                    }

                    // Show the main window
                    if let Some(main_win) = app_handle.get_webview_window("main") {
                        let _ = main_win.show();
                        let _ = main_win.set_focus();
                    }
                });
            }

            debug!(
                "[Startup] setup() hook done (+{}ms from run())",
                app_t0.elapsed().as_millis()
            );
            Ok(())
        })
        .on_window_event(|_window, _event| {
            // Handle window close - hide to tray instead of exiting
            #[cfg(windows)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = _event {
                if _window.label() == "main" {
                    // Hide to tray instead of closing
                    api.prevent_close();
                    let _ = _window.hide();
                }
                // About window and other windows close normally
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
            notify_accessibility_permission_granted,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
