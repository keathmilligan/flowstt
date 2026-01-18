//! IPC client for communicating with the FlowSTT service.
//!
//! This client is used by the Tauri GUI to communicate with the background service.
//! It handles connection management, service auto-spawn, and event forwarding.

use flowstt_common::ipc::{
    get_socket_path, read_json, write_json, EventType, IpcError, Request, Response,
};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

/// IPC client for communicating with the FlowSTT service.
pub struct IpcClient {
    #[cfg(unix)]
    stream: Option<tokio::net::UnixStream>,
    #[cfg(windows)]
    stream: Option<tokio::net::windows::named_pipe::NamedPipeClient>,
}

impl IpcClient {
    /// Create a new client (not connected).
    pub fn new() -> Self {
        Self { stream: None }
    }

    /// Check if the client is connected.
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Connect to the service.
    pub async fn connect(&mut self) -> Result<(), IpcError> {
        let socket_path = get_socket_path();

        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(&socket_path)
                .await
                .map_err(IpcError::Io)?;
            self.stream = Some(stream);
        }

        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;
            let stream = ClientOptions::new()
                .open(&socket_path)
                .map_err(IpcError::Io)?;
            self.stream = Some(stream);
        }

        Ok(())
    }

    /// Disconnect from the service.
    #[allow(dead_code)]
    pub fn disconnect(&mut self) {
        self.stream = None;
    }

    /// Try to connect, spawning the service if needed.
    pub async fn connect_or_spawn(&mut self) -> Result<(), IpcError> {
        // First try to connect
        if self.connect().await.is_ok() {
            return Ok(());
        }

        // Service not running, try to spawn it
        eprintln!("[IpcClient] Service not running, starting...");
        spawn_service()?;

        // Wait for service to be ready (up to 5 seconds)
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if self.connect().await.is_ok() {
                eprintln!("[IpcClient] Service started and connected");
                return Ok(());
            }
        }

        Err(IpcError::ParseError(
            "Service failed to start within timeout".into(),
        ))
    }

    /// Send a request and receive a response.
    pub async fn request(&mut self, request: Request) -> Result<Response, IpcError> {
        // Ensure we're connected
        if self.stream.is_none() {
            self.connect_or_spawn().await?;
        }

        #[cfg(unix)]
        {
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| IpcError::ParseError("Not connected".into()))?;
            let (mut reader, mut writer) = stream.split();
            write_json(&mut writer, &request).await?;
            read_json(&mut reader).await
        }

        #[cfg(windows)]
        {
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| IpcError::ParseError("Not connected".into()))?;
            let (mut reader, mut writer) = tokio::io::split(stream);
            write_json(&mut writer, &request).await?;
            read_json(&mut reader).await
        }
    }

    /// Subscribe to events and forward them to Tauri.
    pub async fn subscribe_and_forward(&mut self, app_handle: AppHandle) -> Result<(), IpcError> {
        // Subscribe to events
        let response = self.request(Request::SubscribeEvents).await?;
        if !matches!(response, Response::Subscribed) {
            return Err(IpcError::ParseError("Failed to subscribe to events".into()));
        }

        // Read events and forward to Tauri
        loop {
            #[cfg(unix)]
            let event_response: Response = {
                let stream = self
                    .stream
                    .as_mut()
                    .ok_or_else(|| IpcError::ParseError("Not connected".into()))?;
                let (mut reader, _) = stream.split();
                read_json(&mut reader).await?
            };

            #[cfg(windows)]
            let event_response: Response = {
                let stream = self
                    .stream
                    .as_mut()
                    .ok_or_else(|| IpcError::ParseError("Not connected".into()))?;
                let (mut reader, _) = tokio::io::split(stream);
                read_json(&mut reader).await?
            };

            match event_response {
                Response::Event { event } => {
                    forward_event_to_tauri(&app_handle, event);
                }
                Response::Error { message } => {
                    eprintln!("[IpcClient] Event stream error: {}", message);
                    break;
                }
                _ => {
                    // Ignore other responses in event stream
                }
            }
        }

        Ok(())
    }
}

/// Forward a service event to the Tauri frontend.
fn forward_event_to_tauri(app_handle: &AppHandle, event: EventType) {
    match event {
        EventType::VisualizationData(data) => {
            // Emit visualization data to frontend
            let _ = app_handle.emit("visualization-data", &data);
        }
        EventType::TranscriptionComplete(result) => {
            let _ = app_handle.emit("transcription-complete", &result.text);
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
            let _ = app_handle.emit("capture-state-changed", CaptureState { capturing, error });
        }
        EventType::ModelDownloadProgress { percent } => {
            let _ = app_handle.emit("model-download-progress", percent);
        }
        EventType::ModelDownloadComplete { success } => {
            let _ = app_handle.emit("model-download-complete", success);
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
        EventType::Shutdown => {
            let _ = app_handle.emit("service-shutdown", ());
        }
    }
}

/// Get the path to the service executable.
fn get_service_path() -> PathBuf {
    // Try to find the service binary next to the GUI binary
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let service_path = dir.join(if cfg!(windows) {
                "flowstt-service.exe"
            } else {
                "flowstt-service"
            });
            if service_path.exists() {
                return service_path;
            }
        }
    }

    // Fall back to PATH
    PathBuf::from(if cfg!(windows) {
        "flowstt-service.exe"
    } else {
        "flowstt-service"
    })
}

/// Spawn the service process.
fn spawn_service() -> Result<(), IpcError> {
    let service_path = get_service_path();

    Command::new(&service_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            IpcError::ParseError(format!(
                "Failed to spawn service at {:?}: {}",
                service_path, e
            ))
        })?;

    Ok(())
}

/// Shared IPC client state for use in Tauri commands.
#[derive(Clone)]
pub struct SharedIpcClient {
    pub client: Arc<Mutex<IpcClient>>,
}

impl SharedIpcClient {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(IpcClient::new())),
        }
    }
}
