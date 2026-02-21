//! IPC client for communicating with the FlowSTT application.

use flowstt_common::ipc::{get_socket_path, read_json, write_json, IpcError, Request, Response};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// IPC client for communicating with the FlowSTT application.
pub struct Client {
    #[cfg(unix)]
    stream: Option<tokio::net::UnixStream>,
    #[cfg(windows)]
    stream: Option<tokio::net::windows::named_pipe::NamedPipeClient>,
}

impl Client {
    /// Create a new client (not connected).
    pub fn new() -> Self {
        Self {
            stream: None,
        }
    }

    /// Connect to the application.
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

    /// Check if the application is running.
    #[allow(dead_code)]
    pub async fn is_app_running() -> bool {
        let socket_path = get_socket_path();
        socket_path.exists()
    }

    /// Try to connect, spawning the application in headless mode if needed.
    /// Returns Ok if connected, Err if connection/spawn failed.
    pub async fn connect_or_spawn(&mut self) -> Result<(), IpcError> {
        // First try to connect
        if self.connect().await.is_ok() {
            return Ok(());
        }

        // Application not running, try to spawn it in headless mode
        eprintln!("Application not running, starting...");
        spawn_app()?;

        // Wait for application to be ready (up to 5 seconds)
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if self.connect().await.is_ok() {
                return Ok(());
            }
        }

        Err(IpcError::ParseError(
            "Application failed to start within timeout".into(),
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

    /// Ping the application.
    pub async fn ping(&mut self) -> Result<bool, IpcError> {
        match self.request(Request::Ping).await? {
            Response::Pong => Ok(true),
            Response::Error { message } => Err(IpcError::ParseError(message)),
            _ => Err(IpcError::ParseError("Unexpected response".into())),
        }
    }

    /// Subscribe to events. After this, use `read_event()` to read events.
    pub async fn subscribe_events(&mut self) -> Result<(), IpcError> {
        let response = self.request(Request::SubscribeEvents).await?;
        match response {
            Response::Subscribed => Ok(()),
            Response::Error { message } => Err(IpcError::ParseError(message)),
            _ => Err(IpcError::ParseError("Failed to subscribe to events".into())),
        }
    }

    /// Read the next event from the stream (blocking).
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

/// Get the path to the application executable.
fn get_app_path() -> PathBuf {
    let app_name = if cfg!(windows) {
        "flowstt-app.exe"
    } else {
        "flowstt-app"
    };

    // macOS: check standard application locations
    #[cfg(target_os = "macos")]
    {
        let mac_app_paths = [
            PathBuf::from("/Applications/FlowSTT.app/Contents/MacOS/flowstt-app"),
            dirs::home_dir()
                .map(|h| h.join("Applications/FlowSTT.app/Contents/MacOS/flowstt-app"))
                .unwrap_or_default(),
        ];
        for path in &mac_app_paths {
            if path.exists() {
                return path.clone();
            }
        }
    }

    // Check next to the CLI binary (development or bundled)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let app_path = dir.join(app_name);
            if app_path.exists() {
                return app_path;
            }
        }
    }

    // Fall back to PATH
    PathBuf::from(app_name)
}

/// Spawn the application process in headless mode.
fn spawn_app() -> Result<Child, IpcError> {
    let app_path = get_app_path();

    Command::new(&app_path)
        .arg("--headless")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            IpcError::ParseError(format!(
                "Failed to spawn application at {:?}: {}",
                app_path, e
            ))
        })
}
