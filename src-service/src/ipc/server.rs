//! IPC server implementation.
//!
//! This module provides the IPC server that handles client connections
//! and routes requests to handlers. It supports both Unix sockets (Linux/macOS)
//! and named pipes (Windows).

use flowstt_common::ipc::{
    get_socket_path, read_json, write_json, EventType, IpcError, Request, Response,
};
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, Mutex};
use tracing::{debug, error, info, warn};

use super::handlers::handle_request;
use crate::is_shutdown_requested;
use crate::state::get_service_state;

/// Event broadcaster for subscribed clients
pub type EventSender = broadcast::Sender<Response>;

/// Get or create the global event broadcaster
static EVENT_BROADCASTER: std::sync::OnceLock<EventSender> = std::sync::OnceLock::new();

/// Get the global event broadcaster for sending events to subscribed clients.
pub fn get_event_sender() -> EventSender {
    EVENT_BROADCASTER
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(100);
            tx
        })
        .clone()
}

/// Broadcast an event to all subscribed clients.
/// When no clients are subscribed, log the event instead of silently dropping it.
pub fn broadcast_event(event: Response) {
    let sender = get_event_sender();
    if sender.receiver_count() == 0 {
        // No clients subscribed - log based on event type
        if let Response::Event { ref event } = event {
            match event {
                EventType::TranscriptionComplete(result) => {
                    info!("Transcription complete (no clients): {}", result.text);
                }
                EventType::VisualizationData(_) => {
                    // High-frequency event - use debug level
                    debug!("Visualization data generated (no clients)");
                }
                EventType::SpeechStarted => {
                    debug!("Speech started (no clients)");
                }
                EventType::SpeechEnded { duration_ms } => {
                    debug!("Speech ended (no clients): {}ms", duration_ms);
                }
                EventType::CaptureStateChanged { capturing, error } => {
                    info!(
                        "Capture state changed (no clients): capturing={}, error={:?}",
                        capturing, error
                    );
                }
                EventType::PttPressed => {
                    info!("PTT pressed (no clients)");
                }
                EventType::PttReleased => {
                    info!("PTT released (no clients)");
                }
                EventType::TranscriptionModeChanged { mode } => {
                    info!("Transcription mode changed (no clients): {:?}", mode);
                }
                EventType::ModelDownloadProgress { percent } => {
                    info!("Model download progress (no clients): {}%", percent);
                }
                EventType::ModelDownloadComplete { success } => {
                    info!("Model download complete (no clients): success={}", success);
                }
                EventType::Shutdown => {
                    info!("Shutdown event (no clients)");
                }
            }
        }
        return;
    }
    // Send to subscribed clients (ignore lagged errors)
    let _ = sender.send(event);
}

/// Run the IPC server until shutdown.
///
/// If `ready_tx` is provided, it is notified once the server is listening and
/// ready to accept client connections. This allows callers to run heavy
/// initialization concurrently without racing the first client connect.
#[cfg(unix)]
pub async fn run_server(ready_tx: Option<oneshot::Sender<()>>) -> Result<(), IpcError> {
    use tokio::net::UnixListener;

    let socket_path = get_socket_path();

    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(IpcError::Io)?;
        }
    }

    // Remove stale socket file if it exists
    if socket_path.exists() {
        info!("Removing stale socket file: {:?}", socket_path);
        std::fs::remove_file(&socket_path).map_err(IpcError::Io)?;
    }

    // Bind to socket
    let listener = UnixListener::bind(&socket_path).map_err(IpcError::Io)?;
    info!("IPC server listening on {:?}", socket_path);

    // Signal readiness - the socket is now bound and accepting connections
    if let Some(tx) = ready_tx {
        let _ = tx.send(());
    }

    loop {
        if is_shutdown_requested() {
            info!("Shutdown requested, stopping IPC server");
            break;
        }

        // Accept connections with timeout for shutdown checking
        let accept_result =
            tokio::time::timeout(std::time::Duration::from_secs(1), listener.accept()).await;

        match accept_result {
            Ok(Ok((stream, _addr))) => {
                info!("Client connected");
                tokio::spawn(async move {
                    if let Err(e) = handle_unix_client(stream).await {
                        if !matches!(e, IpcError::ConnectionClosed) {
                            error!("Client error: {}", e);
                        }
                    }
                    info!("Client disconnected");
                });
            }
            Ok(Err(e)) => {
                error!("Accept error: {}", e);
            }
            Err(_) => {
                // Timeout, check shutdown flag again
                continue;
            }
        }
    }

    Ok(())
}

/// Handle a Unix socket client connection.
#[cfg(unix)]
async fn handle_unix_client(stream: tokio::net::UnixStream) -> Result<(), IpcError> {
    let (reader, writer) = stream.into_split();
    handle_client_connection(reader, writer).await
}

/// Run the IPC server on Windows using named pipes.
///
/// If `ready_tx` is provided, it is notified once the first pipe instance has
/// been created and is ready to accept client connections. This allows callers
/// to run heavy initialization concurrently without racing the first client
/// connect.
#[cfg(windows)]
pub async fn run_server(mut ready_tx: Option<oneshot::Sender<()>>) -> Result<(), IpcError> {
    use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

    let pipe_name = get_socket_path();
    let pipe_name_str = pipe_name.to_string_lossy();
    info!("IPC server listening on {}", pipe_name_str);

    loop {
        if is_shutdown_requested() {
            info!("Shutdown requested, stopping IPC server");
            break;
        }

        // Create a new pipe instance
        let server = match ServerOptions::new()
            .first_pipe_instance(false)
            .pipe_mode(PipeMode::Byte)
            .create(&pipe_name)
        {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create pipe: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        // Signal readiness after the first pipe instance is successfully created.
        // At this point clients can connect via the named pipe.
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(());
        }

        // Wait for client with timeout
        let connect_result =
            tokio::time::timeout(std::time::Duration::from_secs(1), server.connect()).await;

        match connect_result {
            Ok(Ok(())) => {
                info!("Client connected");
                tokio::spawn(async move {
                    if let Err(e) = handle_windows_client(server).await {
                        if !matches!(e, IpcError::ConnectionClosed) {
                            error!("Client error: {}", e);
                        }
                    }
                    info!("Client disconnected");
                });
            }
            Ok(Err(e)) => {
                error!("Pipe connect error: {}", e);
            }
            Err(_) => {
                // Timeout, check shutdown flag again
                continue;
            }
        }
    }

    Ok(())
}

/// Handle a Windows named pipe client connection.
#[cfg(windows)]
async fn handle_windows_client(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
) -> Result<(), IpcError> {
    let (reader, writer) = tokio::io::split(pipe);
    handle_client_connection(reader, writer).await
}

/// Handle a client connection (platform-agnostic).
async fn handle_client_connection<R, W>(reader: R, writer: W) -> Result<(), IpcError>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let reader = Arc::new(Mutex::new(reader));
    let writer = Arc::new(Mutex::new(writer));
    let mut event_receiver: Option<broadcast::Receiver<Response>> = None;
    let mut subscribed = false;

    loop {
        if is_shutdown_requested() {
            // Notify client of shutdown if subscribed
            if subscribed {
                let mut w = writer.lock().await;
                let _ = write_json(
                    &mut *w,
                    &Response::Event {
                        event: flowstt_common::ipc::EventType::Shutdown,
                    },
                )
                .await;
            }
            break;
        }

        // If subscribed to events, use select! to handle both events and requests efficiently
        if subscribed {
            if let Some(ref mut rx) = event_receiver {
                let mut r = reader.lock().await;

                tokio::select! {
                    // Wait for event from broadcast channel
                    event_result = rx.recv() => {
                        drop(r); // Release reader lock before writing
                        match event_result {
                            Ok(event) => {
                                let mut w = writer.lock().await;
                                write_json(&mut *w, &event).await?;
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("Client lagged {} events", n);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                // Channel closed, unsubscribe
                                subscribed = false;
                                event_receiver = None;
                            }
                        }
                        continue;
                    }
                    // Wait for request from client (with longer timeout since events are prioritized)
                    read_result = tokio::time::timeout(std::time::Duration::from_secs(1), read_json(&mut *r)) => {
                        drop(r); // Release reader lock
                        match read_result {
                            Ok(Ok(request)) => {
                                info!("Received request: {:?}", request);
                                let response = handle_request(request).await;
                                info!("Sending response: {:?}", response);
                                let mut w = writer.lock().await;
                                write_json(&mut *w, &response).await?;
                            }
                            Ok(Err(e)) => {
                                return Err(e);
                            }
                            Err(_) => {
                                // Timeout, continue loop to check for events/shutdown
                            }
                        }
                        continue;
                    }
                }
            }
        }

        // Not subscribed - just handle requests with timeout
        let read_result = {
            let mut r = reader.lock().await;
            tokio::time::timeout(std::time::Duration::from_millis(100), read_json(&mut *r)).await
        };

        match read_result {
            Ok(Ok(request)) => {
                info!("Received request: {:?}", request);

                // Check if this is a subscribe request
                let is_subscribe = matches!(request, Request::SubscribeEvents);
                if is_subscribe {
                    subscribed = true;
                    event_receiver = Some(get_event_sender().subscribe());
                }

                // Handle request
                let response = handle_request(request).await;
                info!("Sending response: {:?}", response);

                // Send response
                let mut w = writer.lock().await;
                write_json(&mut *w, &response).await?;

                // After subscribing, send current capture state so the
                // client immediately knows whether transcription is active
                if is_subscribe {
                    let state_arc = get_service_state();
                    let state = state_arc.lock().await;
                    let synthetic = Response::Event {
                        event: EventType::CaptureStateChanged {
                            capturing: state.transcribe_status.capturing,
                            error: state.transcribe_status.error.clone(),
                        },
                    };
                    drop(state);
                    write_json(&mut *w, &synthetic).await?;
                }
            }
            Ok(Err(e)) => {
                // Read error
                return Err(e);
            }
            Err(_) => {
                // Timeout, continue loop
                continue;
            }
        }
    }

    Ok(())
}
