//! FlowSTT Background Service
//!
//! This is the background service that handles all audio capture, processing,
//! and transcription operations. It communicates with CLI and GUI clients via IPC.

mod audio;
mod audio_loop;
pub mod config;
mod hotkey;
mod ipc;
mod platform;
mod processor;
mod ptt_controller;
mod state;
mod transcription;

pub use audio_loop::{
    is_audio_loop_active, start_audio_loop, stop_audio_loop, TranscriptionEventBroadcaster,
};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Global shutdown flag
static SHUTDOWN_FLAG: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

/// Get the global shutdown flag.
pub fn get_shutdown_flag() -> Arc<AtomicBool> {
    SHUTDOWN_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Request service shutdown.
pub fn request_shutdown() {
    info!("Shutdown requested");
    get_shutdown_flag().store(true, Ordering::SeqCst);
}

/// Check if shutdown has been requested.
pub fn is_shutdown_requested() -> bool {
    get_shutdown_flag().load(Ordering::SeqCst)
}

fn main() {
    // Initialize logging with RUST_LOG env var support
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("FlowSTT Service starting (pid: {})...", std::process::id());

    // Load configuration from disk and apply to service state
    let loaded_config = config::Config::load();
    {
        let state = state::get_service_state();
        let mut state = state.blocking_lock();
        state.transcription_mode = loaded_config.transcription_mode;
        state.ptt_key = loaded_config.ptt_key;
        info!(
            "Applied config: transcription_mode={:?}, ptt_key={:?}",
            state.transcription_mode, state.ptt_key
        );
    }

    // Set up signal handlers for graceful shutdown
    setup_signal_handlers();

    // Run async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    runtime.block_on(async {
        // Initialize platform-specific audio backends
        info!("Initializing audio backends...");
        if let Err(e) = platform::init_audio_backend() {
            error!("Failed to initialize audio backend: {}", e);
        }

        // Initialize hotkey backend (non-fatal if unavailable)
        info!("Initializing hotkey backend...");
        if let Err(e) = hotkey::init_hotkey_backend() {
            info!("Hotkey backend not available: {}", e);
        }

        // Initialize transcription system (worker ready to process segments)
        ipc::handlers::init_transcription_system();

        // In PTT mode, configure default audio source and start monitoring
        {
            let state_arc = state::get_service_state();
            let is_ptt_mode = {
                let state = state_arc.lock().await;
                state.transcription_mode == flowstt_common::TranscriptionMode::PushToTalk
            };

            if is_ptt_mode {
                // Get default input device
                let default_source = platform::get_backend().and_then(|b| {
                    let devices = b.list_input_devices();
                    devices.into_iter().next().map(|d| d.id)
                });

                if let Some(source_id) = default_source {
                    info!("PTT mode: Using default audio source: {}", source_id);

                    // Configure state for PTT operation
                    {
                        let mut state = state_arc.lock().await;
                        state.app_ready = true;
                        state.source1_id = Some(source_id);
                    }

                    // Start hotkey monitoring
                    let ptt_key = {
                        let state = state_arc.lock().await;
                        state.ptt_key
                    };

                    info!("PTT mode: Starting hotkey monitoring for {:?}", ptt_key);
                    match hotkey::start_hotkey(ptt_key) {
                        Ok(()) => {
                            // Start PTT controller to handle key events
                            if let Err(e) = ptt_controller::start_ptt_controller() {
                                error!("Failed to start PTT controller: {}", e);
                            } else {
                                info!("PTT mode: Ready - press {:?} to record", ptt_key);
                            }
                        }
                        Err(e) => error!("Failed to start hotkey monitoring: {}", e),
                    }
                } else {
                    warn!("PTT mode: No audio input devices found");
                }
            }
        }

        // Start the IPC server (runs until shutdown)
        if let Err(e) = ipc::run_server().await {
            if !is_shutdown_requested() {
                error!("IPC server error: {}", e);
                std::process::exit(1);
            }
        }
    });

    // Cleanup
    cleanup_on_shutdown();
    info!("FlowSTT Service stopped");
}

/// Set up signal handlers for graceful shutdown.
fn setup_signal_handlers() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        // SIGTERM handler
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut sigterm = signal(SignalKind::terminate()).unwrap();
                let mut sigint = signal(SignalKind::interrupt()).unwrap();
                let mut sighup = signal(SignalKind::hangup()).unwrap();

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM");
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT");
                    }
                    _ = sighup.recv() => {
                        info!("Received SIGHUP");
                    }
                }

                request_shutdown();
            });
        });
    }

    #[cfg(windows)]
    {
        // Windows uses Ctrl+C handler
        ctrlc::set_handler(|| {
            info!("Received Ctrl+C");
            request_shutdown();
        })
        .expect("Error setting Ctrl+C handler");
    }
}

/// Cleanup resources on shutdown.
fn cleanup_on_shutdown() {
    info!("Cleaning up...");

    // Stop any active transcription
    // TODO: Implement cleanup

    // Remove socket file
    #[cfg(unix)]
    {
        let socket_path = flowstt_common::ipc::get_socket_path();
        if socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&socket_path) {
                warn!("Failed to remove socket file: {}", e);
            } else {
                info!("Removed socket file: {:?}", socket_path);
            }
        }
    }
}
