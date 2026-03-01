//! FlowSTT Engine
//!
//! Core engine for FlowSTT voice transcription. Handles audio capture, processing,
//! transcription, hotkey monitoring, clipboard, history, and IPC server.
//!
//! This is a library crate consumed by the Tauri application. The engine runs
//! in-process with the GUI, and also hosts an IPC socket server for CLI clients.

mod audio;
pub mod audio_loop;
pub mod clipboard;
pub mod config;
pub mod history;
pub mod hotkey;
pub mod ipc;
pub mod platform;
pub mod processor;
pub mod ptt_controller;
pub mod state;
pub mod test_capture;
pub mod test_mode;
pub mod transcription;

pub use audio_loop::{
    is_audio_loop_active, start_audio_loop, stop_audio_loop, TranscriptionEventBroadcaster,
};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Global shutdown flag
static SHUTDOWN_FLAG: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

/// Get the global shutdown flag.
pub fn get_shutdown_flag() -> Arc<AtomicBool> {
    SHUTDOWN_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Request engine shutdown.
pub fn request_shutdown() {
    info!("Shutdown requested");
    get_shutdown_flag().store(true, Ordering::SeqCst);
}

/// Check if shutdown has been requested.
pub fn is_shutdown_requested() -> bool {
    get_shutdown_flag().load(Ordering::SeqCst)
}

/// Initialize the engine: load config, history, start IPC server, audio backends,
/// transcription system, and optionally start auto-capture.
///
/// This is called from the Tauri `setup()` hook. It expects to run within
/// an async context (Tokio runtime).
///
/// Returns the IPC server join handle for the caller to manage.
pub async fn init() -> Result<tokio::task::JoinHandle<()>, String> {
    info!("FlowSTT Engine starting (pid: {})...", std::process::id());

    // Detect and store runtime mode
    let runtime_mode = flowstt_common::runtime_mode();
    info!("Runtime mode: {:?}", runtime_mode);
    {
        let state = state::get_service_state();
        let mut state = state.lock().await;
        state.runtime_mode = runtime_mode;
    }

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
            warn!(
                "Failed to create recordings directory {:?}: {}",
                recordings_dir, e
            );
        }
    }

    // Load configuration from disk and apply to engine state
    let loaded_config = config::load_config();
    {
        let state = state::get_service_state();
        let mut state = state.lock().await;
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

    // Start the IPC server so CLI clients can connect immediately.
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let ipc_server_handle = tokio::spawn(async {
        if let Err(e) = ipc::run_server(Some(ready_tx)).await {
            if !is_shutdown_requested() {
                error!("IPC server error: {}", e);
            }
        }
    });

    // Wait until the IPC server is actually listening before proceeding.
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

    // Auto-configure audio sources and start capture immediately,
    // but only if first-time setup is already complete.
    if !first_run {
        let state_arc = state::get_service_state();

        // Resolve primary input device: prefer saved preference, fall back to first available.
        let source1_id = platform::get_backend().and_then(|b| {
            let input_devices = b.list_input_devices();
            if let Some(preferred_id) = loaded_config.preferred_source1_id.as_deref() {
                if let Some(found) = input_devices.iter().find(|d| d.id == preferred_id) {
                    info!("Restoring saved primary audio source: {}", found.id);
                    return Some(found.id.clone());
                }
                warn!(
                    "Saved primary device {:?} not found; falling back to first available",
                    preferred_id
                );
            }
            input_devices.into_iter().next().map(|d| {
                info!("Using default primary audio source: {}", d.id);
                d.id
            })
        });

        // Resolve reference (system) device: prefer saved preference, fall back to None.
        let source2_id = platform::get_backend().and_then(|b| {
            let preferred_id = loaded_config.preferred_source2_id.as_deref()?;
            let system_devices = b.list_system_devices();
            if let Some(found) = system_devices.iter().find(|d| d.id == preferred_id) {
                info!("Restoring saved reference audio source: {}", found.id);
                Some(found.id.clone())
            } else {
                warn!(
                    "Saved reference device {:?} not found; starting with no reference source",
                    preferred_id
                );
                None
            }
        });

        if let Some(source_id) = source1_id {
            // Configure state with resolved sources
            {
                let mut state = state_arc.lock().await;
                state.source1_id = Some(source_id);
                state.source2_id = source2_id;
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

    info!("Engine initialization complete");

    Ok(ipc_server_handle)
}

/// Clean up engine resources on shutdown.
/// Call this when the Tauri app is exiting.
pub fn cleanup() {
    info!("Engine cleanup...");

    // Remove socket file on Unix
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

/// Check GPU status (for diagnostics).
/// Returns Ok(()) on success, Err with details on failure.
pub fn check_gpu() -> Result<String, String> {
    transcription::whisper_ffi::init_library()?;

    let system_info = transcription::whisper_ffi::get_system_info()?;
    let has_cuda = system_info.contains("CUDA");
    let has_metal = system_info.contains("METAL = 1");
    let has_vulkan = system_info.contains("VULKAN = 1");

    let mut result = format!("System Info: {}\n\nGPU Backends:\n", system_info);
    result.push_str(&format!(
        "  CUDA:   {}\n  Metal:  {}\n  Vulkan: {}",
        if has_cuda { "YES" } else { "NO" },
        if has_metal { "YES" } else { "NO" },
        if has_vulkan { "YES" } else { "NO" }
    ));

    Ok(result)
}
