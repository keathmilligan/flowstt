//! FlowSTT Background Service
//!
//! This is the background service that handles all audio capture, processing,
//! and transcription operations. It communicates with CLI and GUI clients via IPC.

mod audio;
mod audio_loop;
mod clipboard;
pub mod config;
pub mod history;
mod hotkey;
mod ipc;
mod platform;
mod processor;
mod ptt_controller;
mod state;
mod test_capture;
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
    // Check for --check-gpu flag for quick GPU diagnostics
    let check_gpu = std::env::args().any(|arg| arg == "--check-gpu");

    // Initialize logging with RUST_LOG env var support
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // If --check-gpu, just initialize whisper and print GPU status, then exit
    if check_gpu {
        println!("=== GPU Check Mode ===");
        println!("Initializing whisper library and checking GPU status...\n");

        // Initialize the whisper library (this loads ggml backends)
        match transcription::whisper_ffi::init_library() {
            Ok(()) => println!("Whisper library initialized successfully"),
            Err(e) => {
                println!("ERROR: Failed to initialize whisper library: {}", e);
                std::process::exit(1);
            }
        }

        // Get system info
        match transcription::whisper_ffi::get_system_info() {
            Ok(info) => {
                println!("\nSystem Info: {}", info);
                let has_cuda = info.contains("CUDA");
                let has_metal = info.contains("METAL = 1");
                let has_vulkan = info.contains("VULKAN = 1");
                println!("\nGPU Backends:");
                println!("  CUDA:   {}", if has_cuda { "YES" } else { "NO" });
                println!("  Metal:  {}", if has_metal { "YES" } else { "NO" });
                println!("  Vulkan: {}", if has_vulkan { "YES" } else { "NO" });
            }
            Err(e) => println!("ERROR: Failed to get system info: {}", e),
        }

        // Try to load the model to trigger full GPU initialization
        println!("\nAttempting to load whisper model...");
        let mut transcriber = transcription::Transcriber::new();
        if transcriber.is_model_available() {
            match transcriber.load_model() {
                Ok(()) => println!("Model loaded successfully"),
                Err(e) => println!("ERROR: Failed to load model: {}", e),
            }
        } else {
            println!("Model not found at: {:?}", transcriber.get_model_path());
        }

        println!("\n=== GPU Check Complete ===");
        return;
    }

    info!("FlowSTT Service starting (pid: {})...", std::process::id());

    // Load transcription history and clean up old WAV files (>24h)
    {
        let history = history::get_history();
        let mut h = history.lock().unwrap();
        info!("Loaded {} history entries", h.get_entries().len());
        h.cleanup_wav_files(std::time::Duration::from_secs(86400));
    }

    // Ensure recordings directory exists
    {
        let recordings_dir = history::TranscriptionHistory::recordings_dir();
        if let Err(e) = std::fs::create_dir_all(&recordings_dir) {
            warn!("Failed to create recordings directory {:?}: {}", recordings_dir, e);
        }
    }

    // Load configuration from disk and apply to service state
    let loaded_config = config::load_config();
    {
        let state = state::get_service_state();
        let mut state = state.blocking_lock();
        state.transcription_mode = loaded_config.transcription_mode;
        state.ptt_hotkeys = loaded_config.ptt_hotkeys.clone();
        state.auto_toggle_hotkeys = loaded_config.auto_toggle_hotkeys.clone();
        info!(
            "Applied config: transcription_mode={:?}, ptt_hotkeys={} combination(s), auto_toggle_hotkeys={} combination(s)",
            state.transcription_mode,
            state.ptt_hotkeys.len(),
            state.auto_toggle_hotkeys.len()
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
        // Start the IPC server first so clients can connect immediately.
        // Heavy subsystem initialization (audio, transcription) runs concurrently
        // below. IPC handlers already handle the case where backends aren't ready
        // yet (e.g. ListDevices returns empty, GetStatus shows not capturing).
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let ipc_server_handle = tokio::spawn(async {
            if let Err(e) = ipc::run_server(Some(ready_tx)).await {
                if !is_shutdown_requested() {
                    error!("IPC server error: {}", e);
                    std::process::exit(1);
                }
            }
        });

        // Wait until the IPC server is actually listening before proceeding.
        // This ensures the named pipe / socket exists when we continue, so any
        // client that was spawned alongside us can connect immediately.
        let _ = ready_rx.await;

        // Initialize platform-specific audio backends
        info!("Initializing audio backends...");
        if let Err(e) = platform::init_audio_backend() {
            error!("Failed to initialize audio backend: {}", e);
        }

        // Initialize transcription system (worker ready to process segments)
        ipc::handlers::init_transcription_system();

        // During first-time setup, skip hotkey initialization and auto-capture
        // entirely. The setup wizard will explicitly start capture (and thus
        // hotkey listening) only when the user reaches the test page.
        let first_run = flowstt_common::config::Config::needs_setup();

        if !first_run {
            // Initialize hotkey backend (non-fatal if unavailable)
            info!("Initializing hotkey backend...");
            if let Err(e) = hotkey::init_hotkey_backend() {
                info!("Hotkey backend not available: {}", e);
            }
        }

        // Auto-configure default audio source and start capture immediately,
        // but only if first-time setup is already complete.
        if !first_run {
            let state_arc = state::get_service_state();

            // Get default input device
            let default_source = platform::get_backend().and_then(|b| {
                let devices = b.list_input_devices();
                devices.into_iter().next().map(|d| d.id)
            });

            if let Some(source_id) = default_source {
                info!("Using default audio source: {}", source_id);

                // Configure state with default source
                {
                    let mut state = state_arc.lock().await;
                    state.source1_id = Some(source_id);
                }

                // Start capture (handles both Automatic and PTT modes)
                match ipc::handlers::start_capture().await {
                    Ok(()) => {
                        let state = state_arc.lock().await;
                        info!("Capture started in {:?} mode", state.transcription_mode);
                    }
                    Err(e) => error!("Failed to start capture: {}", e),
                }
            } else {
                warn!("No audio input devices found; waiting for client to configure via SetSources");
            }
        } else {
            info!("First-time setup pending; skipping auto-capture (waiting for setup wizard)");
        }

        info!("Service initialization complete");

        // Wait for IPC server to finish (runs until shutdown)
        let _ = ipc_server_handle.await;
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
