//! FlowSTT GUI - Tauri application with integrated engine.
//!
//! The engine (audio capture, transcription, IPC server) runs in-process.
//! Tauri commands call engine functions directly without IPC serialization.
//! The IPC socket server is hosted by this process for CLI client access.

mod tray;

use flowstt_common::config::{Config, LogLevel, ThemeMode};
use flowstt_common::ipc::{EventType, Request, Response};
use flowstt_common::{
    runtime_mode, AudioDevice, HotkeyCombination, RecordingMode, RuntimeMode, TranscriptionMode,
};
use std::env;
use std::sync::Arc;
use std::time::Instant;
use tauri::webview::WebviewWindowBuilder;
use tauri::WebviewUrl;
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{reload, EnvFilter};

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

// ─── Log line payload (emitted to frontend) ──────────────────────────────────

/// A single pre-formatted log line sent to the frontend log viewer.
///
/// The line is formatted identically to what `tracing_subscriber::fmt` writes
/// to the log file, so history (read from file) and live events render the same.
///
/// Format: `{timestamp}  {LEVEL} {target}: {message}\n`
/// e.g.  `2026-03-02T00:27:33.464210Z  INFO flowstt_lib: engine started`
#[derive(serde::Serialize, Clone)]
struct LogLinePayload {
    line: String,
}

// ─── TauriLogLayer ───────────────────────────────────────────────────────────

/// A `tracing_subscriber::Layer` that forwards log events to the frontend
/// via a bounded mpsc channel. The channel receiver is drained by a task
/// spawned once `AppHandle` is available (after `Builder::build()`).
struct TauriLogLayer {
    sender: tokio::sync::mpsc::Sender<LogLinePayload>,
}

impl<S> tracing_subscriber::Layer<S> for TauriLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use tracing_subscriber::field::Visit;

        struct MessageVisitor(String);
        impl Visit for MessageVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{:?}", value);
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.0 = value.to_string();
                }
            }
        }

        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);

        // Format identically to tracing_subscriber::fmt's default output so that
        // live events and file-read history look the same in the log viewer.
        // File format: "2026-03-02T00:27:33.464210Z  INFO flowstt_lib: message"
        let now = chrono::Utc::now();
        let level = event.metadata().level().to_string().to_uppercase();
        let target = event.metadata().target();
        // Pad level to 5 chars (matching tracing_subscriber's default alignment)
        let line = format!(
            "{}  {:5} {}: {}",
            now.format("%Y-%m-%dT%H:%M:%S%.6fZ"),
            level,
            target,
            visitor.0,
        );
        let payload = LogLinePayload { line };

        // Non-blocking send; drop if channel is full to avoid blocking the caller.
        let _ = self.sender.try_send(payload);
    }
}

// ─── Log state (reload handle + channel sender) ──────────────────────────────

/// State holding the runtime-reloadable log filter handle and log-line sender.
struct LogState {
    /// Allows reloading the `EnvFilter` at runtime (e.g. from `set_log_level`).
    reload_handle: Arc<reload::Handle<EnvFilter, tracing_subscriber::Registry>>,
}

/// Initialize the layered tracing subscriber.
///
/// Returns:
/// - A `LogState` containing the reload handle (stored in Tauri state).
/// - An mpsc receiver for log lines (consumed by a forwarder task after app build).
fn init_logging(initial_level: &LogLevel) -> (LogState, tokio::sync::mpsc::Receiver<LogLinePayload>) {
    let mode = runtime_mode();
    let filter_str = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(initial_level.as_filter_str()));

    let (filter_layer, reload_handle) = reload::Layer::new(filter_str);

    // Bounded channel: 1000 log lines buffered for frontend forwarding.
    let (tx, rx) = tokio::sync::mpsc::channel::<LogLinePayload>(1000);
    let tauri_layer = TauriLogLayer { sender: tx };

    // Both production and development write to the log file.
    // Development additionally writes to stdout with ANSI color.
    if let Err(e) = flowstt_common::logging::ensure_log_dir() {
        // pre-subscriber bootstrap: cannot use tracing yet
        eprintln!(
            "Warning: Failed to create log directory, using temp dir: {}",
            e
        );
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
            // pre-subscriber bootstrap: cannot use tracing yet
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

    // Use the rolling appender directly (synchronous) instead of non_blocking.
    // Non-blocking buffers writes in a background channel; forgetting the guard
    // means that buffer is never flushed, so the log file is empty until the
    // channel fills or the app exits. Synchronous writes go to the OS
    // immediately, so get_log_history can read them right away.
    let file_fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false);

    match mode {
        RuntimeMode::Production => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(file_fmt_layer)
                .with(tauri_layer)
                .init();
        }
        RuntimeMode::Development => {
            // In development: also write to stdout with ANSI color for live terminal feedback.
            let stdout_fmt_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true);

            tracing_subscriber::registry()
                .with(filter_layer)
                .with(file_fmt_layer)
                .with(stdout_fmt_layer)
                .with(tauri_layer)
                .init();
        }
    }

    let log_state = LogState {
        reload_handle: Arc::new(reload_handle),
    };

    (log_state, rx)
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
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::ListDevices { source_type: None })
            .await;
    match response {
        Response::Devices { devices } => Ok(devices),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Set audio sources
#[tauri::command]
async fn set_sources(source1_id: Option<String>, source2_id: Option<String>) -> Result<(), String> {
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
    let response = flowstt_engine::ipc::handlers::handle_request(Request::GetModelStatus).await;
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
    let response = flowstt_engine::ipc::handlers::handle_request(Request::DownloadModel).await;
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
    let response = flowstt_engine::ipc::handlers::handle_request(Request::GetCudaStatus).await;
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
    let response = flowstt_engine::ipc::handlers::handle_request(Request::GetStatus).await;
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
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::SetTranscriptionMode { mode }).await;
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
    app_handle: AppHandle,
) -> Result<(), String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::SetPushToTalkHotkeys { hotkeys })
            .await;
    match response {
        Response::Ok => {
            let _ = app_handle.emit("ptt-hotkeys-changed", ());
            Ok(())
        }
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Get push-to-talk status
#[tauri::command]
async fn get_ptt_status() -> Result<LocalPttStatus, String> {
    let response = flowstt_engine::ipc::handlers::handle_request(Request::GetPttStatus).await;
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
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::SetAutoToggleHotkeys { hotkeys })
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
    let response = flowstt_engine::ipc::handlers::handle_request(Request::ToggleAutoMode).await;
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
    let response = flowstt_engine::ipc::handlers::handle_request(Request::GetHistory).await;
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
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::DeleteHistoryEntry { id }).await;
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
    config
        .save()
        .map_err(|e| format!("Failed to save config: {}", e))?;
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
    let _ = flowstt_engine::ipc::handlers::handle_request(Request::SetTranscriptionMode {
        mode: transcription_mode,
    })
    .await;

    app_handle
        .emit("setup-complete", ())
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Start a test audio capture on a device for level metering.
#[tauri::command]
async fn test_audio_device(device_id: String) -> Result<(), String> {
    let response =
        flowstt_engine::ipc::handlers::handle_request(Request::TestAudioDevice { device_id }).await;
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

/// Return the raw text of the current session's log file.
///
/// Finds the most recently modified `flowstt-app.*.log` file in the log
/// directory — this is the file the rolling appender is currently writing to.
/// Returns an empty string if no log file exists yet.
#[tauri::command]
fn get_log_history() -> String {
    let log_dir = flowstt_common::logging::app_log_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("flowstt-logs"));

    // Find the most recently modified flowstt-app.*.log file.
    let most_recent = std::fs::read_dir(&log_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("flowstt-app.") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?;
            Some((e.path(), modified))
        })
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path);

    match most_recent {
        Some(path) => std::fs::read_to_string(&path).unwrap_or_default(),
        None => String::new(),
    }
}

/// Get the current log level from config.
#[tauri::command]
fn get_log_level() -> Result<String, String> {
    let config = Config::load();
    Ok(config.log_level.as_filter_str().to_string())
}

/// Set the minimum log level at runtime and persist to config.
#[tauri::command]
fn set_log_level(level: String, state: State<LogState>) -> Result<(), String> {
    let log_level: LogLevel = match level.as_str() {
        "error" => LogLevel::Error,
        "warn" => LogLevel::Warn,
        "info" => LogLevel::Info,
        "debug" => LogLevel::Debug,
        "trace" => LogLevel::Trace,
        other => return Err(format!("Unknown log level: {}", other)),
    };

    // Reload the subscriber filter immediately.
    state
        .reload_handle
        .reload(EnvFilter::new(log_level.as_filter_str()))
        .map_err(|e| format!("Failed to reload log filter: {}", e))?;

    // Persist to config.
    let mut config = Config::load();
    config.log_level = log_level;
    config
        .save()
        .map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}

/// Download all log files as a zip archive via a native save dialog.
#[tauri::command]
async fn download_logs(app_handle: AppHandle) -> Result<(), String> {
    use std::io::Write;
    use tauri_plugin_dialog::DialogExt;

    let log_dir = flowstt_common::logging::app_log_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("flowstt-logs"));

    // Collect *.log files.
    let entries: Vec<std::path::PathBuf> = match std::fs::read_dir(&log_dir) {
        Ok(dir) => dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("log"))
            .collect(),
        Err(_) => vec![],
    };

    if entries.is_empty() {
        return Err("no_logs".to_string());
    }

    // Build zip in memory.
    let mut zip_buf = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut zip = zip::ZipWriter::new(&mut zip_buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for path in &entries {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(contents) = std::fs::read(path) {
                    let _ = zip.start_file(name, options);
                    let _ = zip.write_all(&contents);
                }
            }
        }
        zip.finish().map_err(|e| format!("Zip error: {}", e))?;
    }

    let zip_bytes = zip_buf.into_inner();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_name = format!("flowstt-logs-{}.zip", today);

    // Show native save dialog and write the bytes.
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<std::path::PathBuf>>();
    app_handle
        .dialog()
        .file()
        .set_file_name(&default_name)
        .save_file(move |path| {
            let _ = tx.send(path.and_then(|p| p.into_path().ok()));
        });

    match rx.await {
        Ok(Some(dest)) => {
            std::fs::write(&dest, &zip_bytes)
                .map_err(|e| format!("Failed to write zip: {}", e))?;
        }
        Ok(None) => {
            // User cancelled — not an error.
        }
        Err(_) => return Err("Dialog channel error".to_string()),
    }

    Ok(())
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

// ─── Window helpers ──────────────────────────────────────────────────────────

/// Open the log viewer window, or focus it if already open.
pub fn open_log_viewer_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("logs") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(app, "logs", WebviewUrl::App("logs.html".into()))
        .title("FlowSTT Logs")
        .inner_size(900.0, 600.0)
        .min_inner_size(600.0, 400.0)
        .resizable(true)
        .decorations(false)
        .transparent(false)
        .shadow(true)
        .skip_taskbar(true)
        .center()
        .build();
}

// ─── Application entry point ─────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_t0 = Instant::now();

    // Parse --headless and --test-mode flags
    let headless = std::env::args().any(|arg| arg == "--headless");
    let test_mode = std::env::args().any(|arg| arg == "--test-mode");

    // Read config before initializing logging so we can use the configured level.
    let initial_config = Config::load();

    // Initialize layered logging subscriber with reloadable filter.
    let (log_state, log_rx) = init_logging(&initial_config.log_level);

    // Set test mode state before tray setup so conditional menu items are available
    if test_mode {
        flowstt_engine::test_mode::set_test_mode(true);
        info!("[Startup] Test mode activated");
    }

    info!(
        "[Startup] run() entered (headless={}, test_mode={})",
        headless, test_mode
    );
    configure_wayland_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            ipc_server_handle: Mutex::new(None),
        })
        .manage(log_state)
        .setup(move |app| {
            // Spawn the log-line forwarder task: drains the mpsc channel and
            // emits each line as a "log-line" Tauri event to all windows.
            {
                let app_handle = app.handle().clone();
                let mut rx = log_rx;
                tauri::async_runtime::spawn(async move {
                    while let Some(payload) = rx.recv().await {
                        let _ = app_handle.emit("log-line", payload);
                    }
                });
            }
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
            let ipc_handle = tauri::async_runtime::block_on(async { flowstt_engine::init().await });

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

            // Restore always-on-top state from config
            {
                let config = Config::load();
                if config.always_on_top {
                    if let Some(main_win) = app.get_webview_window("main") {
                        if let Err(e) = main_win.set_always_on_top(true) {
                            warn!("[Startup] Failed to restore always-on-top: {}", e);
                        }
                    }
                }
            }

            // First-run detection: show setup wizard if no config exists
            if Config::needs_setup() && !headless {
                info!("[Startup] First run detected - showing setup wizard");

                if let Some(main_win) = app.get_webview_window("main") {
                    let _ = main_win.hide();
                }

                let _setup_win =
                    WebviewWindowBuilder::new(app, "setup", WebviewUrl::App("setup.html".into()))
                        .title("FlowSTT Setup")
                        .inner_size(600.0, 500.0)
                        .min_inner_size(500.0, 400.0)
                        .center()
                        .decorations(true)
                        .transparent(false)
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
            get_log_history,
            get_log_level,
            set_log_level,
            download_logs,
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
