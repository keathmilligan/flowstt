//! IPC client for communicating with the FlowSTT service.

use flowstt_common::ipc::{get_socket_path, read_json, write_json, IpcError, Request, Response};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// IPC client for communicating with the FlowSTT service.
pub struct Client {
    #[cfg(unix)]
    stream: Option<tokio::net::UnixStream>,
    #[cfg(windows)]
    stream: Option<tokio::net::windows::named_pipe::NamedPipeClient>,
}

impl Client {
    /// Create a new client (not connected).
    pub fn new() -> Self {
        Self { stream: None }
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

    /// Check if the service is running.
    #[allow(dead_code)]
    pub async fn is_service_running() -> bool {
        let socket_path = get_socket_path();
        socket_path.exists()
    }

    /// Try to connect, spawning the service if needed.
    pub async fn connect_or_spawn(&mut self) -> Result<(), IpcError> {
        // First try to connect
        if self.connect().await.is_ok() {
            return Ok(());
        }

        // Service not running, try to spawn it
        eprintln!("Service not running, starting...");
        spawn_service()?;

        // Wait for service to be ready (up to 5 seconds)
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if self.connect().await.is_ok() {
                return Ok(());
            }
        }

        Err(IpcError::ParseError(
            "Service failed to start within timeout".into(),
        ))
    }

    /// Send a request and receive a response.
    pub async fn request(&mut self, request: Request) -> Result<Response, IpcError> {
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

    /// Ping the service.
    pub async fn ping(&mut self) -> Result<bool, IpcError> {
        match self.request(Request::Ping).await? {
            Response::Pong => Ok(true),
            Response::Error { message } => Err(IpcError::ParseError(message)),
            _ => Err(IpcError::ParseError("Unexpected response".into())),
        }
    }

    /// Subscribe to events. After this, use `read_event()` to read events.
    /// This connection becomes dedicated to event streaming.
    pub async fn subscribe_events(&mut self) -> Result<(), IpcError> {
        let response = self.request(Request::SubscribeEvents).await?;
        match response {
            Response::Subscribed => Ok(()),
            Response::Error { message } => Err(IpcError::ParseError(message)),
            _ => Err(IpcError::ParseError("Failed to subscribe to events".into())),
        }
    }

    /// Read the next event from the stream (blocking).
    /// Must call `subscribe_events()` first.
    pub async fn read_event(&mut self) -> Result<Response, IpcError> {
        #[cfg(unix)]
        {
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| IpcError::ParseError("Not connected".into()))?;
            let (mut reader, _) = stream.split();
            read_json(&mut reader).await
        }

        #[cfg(windows)]
        {
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| IpcError::ParseError("Not connected".into()))?;
            let (mut reader, _) = tokio::io::split(stream);
            read_json(&mut reader).await
        }
    }
}

/// Get the path to the service executable.
fn get_service_path() -> PathBuf {
    // First, check the standard binaries directory (for installed apps)
    let data_dir = directories::BaseDirs::new()
        .map(|b| b.data_local_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let binaries_dir = data_dir.join("FlowSTT").join("bin");
    let extracted_path = binaries_dir.join(if cfg!(windows) {
        "flowstt-service.exe"
    } else {
        "flowstt-service"
    });
    if extracted_path.exists() {
        return extracted_path;
    }

    // Try to find the service binary next to the CLI binary (development or bundled)
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
fn spawn_service() -> Result<Child, IpcError> {
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
        })
}
