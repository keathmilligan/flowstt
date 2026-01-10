//! FlowSTT GUI - Tauri application that communicates with the background service.
//!
//! This module provides the Tauri commands that the frontend uses.
//! All audio capture and transcription is handled by the service via IPC.

mod ipc_client;

use flowstt_common::ipc::{Request, Response};
use flowstt_common::{AudioDevice, RecordingMode};
use ipc_client::{IpcClient, SharedIpcClient};
use std::env;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

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

/// Application state shared between Tauri commands.
struct AppState {
    /// Shared IPC client for communication with the service
    ipc: SharedIpcClient,
    /// Handle to the event forwarding task
    event_task_running: Arc<Mutex<bool>>,
}

/// Helper to send a request to the service and handle errors.
async fn send_request(ipc: &SharedIpcClient, request: Request) -> Result<Response, String> {
    let mut client = ipc.client.lock().await;
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

/// Start monitoring with up to two sources mixed together
#[tauri::command]
async fn start_monitor(
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Need at least one source
    if source1_id.is_none() && source2_id.is_none() {
        return Err("At least one audio source must be selected".to_string());
    }

    // Start monitoring only (transcription disabled - just visualization)
    let response = send_request(
        &state.ipc,
        Request::StartTranscribe {
            source1_id,
            source2_id,
            aec_enabled: false,
            mode: RecordingMode::Mixed,
            transcription_enabled: false,
        },
    )
    .await?;

    match response {
        Response::Ok => {
            // Start event forwarding if not already running
            start_event_forwarding(
                state.ipc.clone(),
                app_handle,
                state.event_task_running.clone(),
            )
            .await;
            Ok(())
        }
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Stop monitoring
#[tauri::command]
async fn stop_monitor(state: State<'_, AppState>) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::StopTranscribe).await?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => {
            if message.contains("not active") {
                Ok(()) // Already stopped
            } else {
                Err(message)
            }
        }
        _ => Err("Unexpected response".into()),
    }
}

/// Check if monitoring is active
#[tauri::command]
async fn is_monitoring(state: State<'_, AppState>) -> Result<bool, String> {
    let response = send_request(&state.ipc, Request::GetStatus).await?;

    match response {
        Response::Status(status) => Ok(status.active),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Start recording with up to two sources mixed together
/// Note: In the new architecture, recording is handled by transcribe mode
#[tauri::command]
async fn start_recording(
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    start_monitor(source1_id, source2_id, state, app_handle).await
}

/// Stop recording
#[tauri::command]
async fn stop_recording(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
    _keep_monitoring: bool,
) -> Result<(), String> {
    stop_monitor(state).await
}

/// Check if recording is active
#[tauri::command]
async fn is_recording(state: State<'_, AppState>) -> Result<bool, String> {
    is_monitoring(state).await
}

/// Set echo cancellation enabled/disabled
#[tauri::command]
async fn set_aec_enabled(_enabled: bool, _state: State<'_, AppState>) -> Result<(), String> {
    // AEC is configured per-request in the service
    // This is now a no-op - AEC is enabled via start_transcribe_mode
    Ok(())
}

/// Check if AEC is enabled
#[tauri::command]
async fn is_aec_enabled(_state: State<'_, AppState>) -> Result<bool, String> {
    // AEC state is managed per-session in the service
    Ok(false)
}

/// Set recording mode
#[tauri::command]
async fn set_recording_mode(
    _mode: RecordingMode,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    // Recording mode is configured per-request in the service
    Ok(())
}

/// Get current recording mode
#[tauri::command]
async fn get_recording_mode(_state: State<'_, AppState>) -> Result<RecordingMode, String> {
    Ok(RecordingMode::Mixed)
}

/// Transcribe audio data (legacy - now handled automatically by transcribe mode)
#[tauri::command]
async fn transcribe(_audio_data: Vec<f32>, _state: State<'_, AppState>) -> Result<String, String> {
    Err("Direct transcription not supported - use transcribe mode".into())
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

/// Transcribe mode status for frontend
#[derive(serde::Serialize)]
struct TranscribeModeStatus {
    active: bool,
    in_speech: bool,
    queue_depth: usize,
}

/// Start automatic transcription mode
#[tauri::command]
async fn start_transcribe_mode(
    source1_id: Option<String>,
    source2_id: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Need at least one source
    if source1_id.is_none() && source2_id.is_none() {
        return Err("At least one audio source must be selected".to_string());
    }

    let response = send_request(
        &state.ipc,
        Request::StartTranscribe {
            source1_id,
            source2_id,
            aec_enabled: false,
            mode: RecordingMode::Mixed,
            transcription_enabled: true,
        },
    )
    .await?;

    match response {
        Response::Ok => {
            // Start event forwarding if not already running
            start_event_forwarding(
                state.ipc.clone(),
                app_handle,
                state.event_task_running.clone(),
            )
            .await;
            println!("[TranscribeMode] Started via service");
            Ok(())
        }
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Stop automatic transcription mode
#[tauri::command]
async fn stop_transcribe_mode(
    state: State<'_, AppState>,
    _app_handle: AppHandle,
) -> Result<(), String> {
    let response = send_request(&state.ipc, Request::StopTranscribe).await?;

    match response {
        Response::Ok => {
            println!("[TranscribeMode] Stopped via service");
            Ok(())
        }
        Response::Error { message } => {
            if message.contains("not active") {
                Ok(()) // Already stopped
            } else {
                Err(message)
            }
        }
        _ => Err("Unexpected response".into()),
    }
}

/// Check if transcribe mode is active
#[tauri::command]
async fn is_transcribe_active(state: State<'_, AppState>) -> Result<bool, String> {
    let response = send_request(&state.ipc, Request::GetStatus).await?;

    match response {
        Response::Status(status) => Ok(status.active),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Get transcribe mode status
#[tauri::command]
async fn get_transcribe_status(state: State<'_, AppState>) -> Result<TranscribeModeStatus, String> {
    let response = send_request(&state.ipc, Request::GetStatus).await?;

    match response {
        Response::Status(status) => Ok(TranscribeModeStatus {
            active: status.active,
            in_speech: status.in_speech,
            queue_depth: status.queue_depth,
        }),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".into()),
    }
}

/// Start the event forwarding task.
/// This subscribes to service events and forwards them to the Tauri frontend.
async fn start_event_forwarding(
    _ipc: SharedIpcClient,
    app_handle: AppHandle,
    running: Arc<Mutex<bool>>,
) {
    // Check if already running
    {
        let is_running = running.lock().await;
        if *is_running {
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
        // Create a dedicated client for event streaming
        let mut event_client = IpcClient::new();

        if let Err(e) = event_client.connect_or_spawn().await {
            eprintln!("[EventForwarder] Failed to connect: {}", e);
            let mut is_running = running_clone.lock().await;
            *is_running = false;
            return;
        }

        // This will run until the connection is closed
        if let Err(e) = event_client.subscribe_and_forward(app_handle).await {
            eprintln!("[EventForwarder] Event stream ended: {}", e);
        }

        // Mark as not running
        let mut is_running = running_clone.lock().await;
        *is_running = false;
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure_wayland_workarounds();

    tauri::Builder::default()
        .manage(AppState {
            ipc: SharedIpcClient::new(),
            event_task_running: Arc::new(Mutex::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            list_all_sources,
            start_recording,
            stop_recording,
            is_recording,
            start_monitor,
            stop_monitor,
            is_monitoring,
            set_aec_enabled,
            is_aec_enabled,
            set_recording_mode,
            get_recording_mode,
            transcribe,
            check_model_status,
            download_model,
            start_transcribe_mode,
            stop_transcribe_mode,
            is_transcribe_active,
            get_transcribe_status,
            get_cuda_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
