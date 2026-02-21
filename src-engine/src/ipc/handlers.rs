//! IPC request handlers.

use flowstt_common::ipc::{EventType, Request, Response};
use flowstt_common::{ConfigValues, CudaStatus, ModelStatus, PttStatus, TranscriptionMode};
use std::sync::Arc;
use tracing::{info, warn};

use super::broadcast_event;
use crate::hotkey;
use crate::platform;
use crate::ptt_controller;
use crate::state::get_service_state;
use crate::transcription::{download_model, TranscribeState, Transcriber, TranscriptionQueue};
use crate::{
    is_audio_loop_active, start_audio_loop, stop_audio_loop, TranscriptionEventBroadcaster,
};

/// Global transcription queue
static TRANSCRIPTION_QUEUE: std::sync::OnceLock<Arc<TranscriptionQueue>> =
    std::sync::OnceLock::new();

pub fn get_transcription_queue() -> Arc<TranscriptionQueue> {
    TRANSCRIPTION_QUEUE
        .get_or_init(|| Arc::new(TranscriptionQueue::new()))
        .clone()
}

/// Global transcribe state
static TRANSCRIBE_STATE: std::sync::OnceLock<Arc<std::sync::Mutex<TranscribeState>>> =
    std::sync::OnceLock::new();

pub fn get_transcribe_state() -> Arc<std::sync::Mutex<TranscribeState>> {
    TRANSCRIBE_STATE
        .get_or_init(|| {
            let queue = get_transcription_queue();
            Arc::new(std::sync::Mutex::new(TranscribeState::new(queue)))
        })
        .clone()
}

/// Initialize the transcription system at startup.
/// Called once when the service starts - sets up the transcription worker
/// so it's ready when audio sources are configured.
pub fn init_transcription_system() {
    info!("Initializing transcription system...");

    // Set up transcription queue callback
    let queue = get_transcription_queue();
    queue.set_callback(Arc::new(TranscriptionEventBroadcaster));

    // Start transcription worker
    let transcriber = Transcriber::new();
    let model_path = transcriber.get_model_path().clone();
    queue.start_worker(model_path);

    info!("Transcription system initialized");
}

/// Start audio capture with current source configuration.
/// Returns Ok if capture started, Err with message if it failed.
pub async fn start_capture() -> Result<(), String> {
    let state_arc = get_service_state();
    let state = state_arc.lock().await;

    if !state.has_primary_source() {
        return Err("No primary audio source configured".to_string());
    }

    let source1_id = state.source1_id.clone();
    let source2_id = state.source2_id.clone(); // Optional
    let aec_enabled = state.aec_enabled;
    let recording_mode = state.recording_mode;
    let transcription_mode = state.transcription_mode;
    let ptt_hotkeys = state.ptt_hotkeys.clone();
    let auto_toggle_hotkeys = state.auto_toggle_hotkeys.clone();

    // Drop the lock before doing expensive operations
    drop(state);

    if transcription_mode == TranscriptionMode::PushToTalk {
        // PTT mode: Don't start audio capture yet, just start the PTT controller
        // Audio will be started/stopped when the hotkey is pressed/released

        // Start hotkey backend
        if let Err(e) = hotkey::start_hotkey(ptt_hotkeys.clone(), auto_toggle_hotkeys.clone()) {
            return Err(format!("Failed to start PTT hotkey monitoring: {}", e));
        }
        info!(
            "PTT hotkey monitoring started for {} combination(s)",
            ptt_hotkeys.len()
        );

        // Start PTT controller
        if let Err(e) = ptt_controller::start_ptt_controller() {
            hotkey::stop_hotkey();
            return Err(format!("Failed to start PTT controller: {}", e));
        }

        // Update state - not capturing yet, but ready
        let state_arc = get_service_state();
        let mut state = state_arc.lock().await;
        state.transcribe_status.capturing = false;
        state.transcribe_status.error = None;

        info!("PTT mode ready - waiting for hotkey press");

        // Broadcast ready event
        broadcast_event(Response::Event {
            event: EventType::CaptureStateChanged {
                capturing: false,
                error: None,
            },
        });

        Ok(())
    } else {
        // Automatic mode: Start continuous audio capture with VAD
        // Also start hotkey backend for toggle hotkey support

        // Start hotkey backend (with toggle hotkeys, empty PTT hotkeys)
        // Only start if toggle hotkeys are configured
        if !auto_toggle_hotkeys.is_empty() {
            if let Err(e) = hotkey::start_hotkey(vec![], auto_toggle_hotkeys.clone()) {
                warn!("Failed to start toggle hotkey monitoring: {}", e);
            } else {
                info!(
                    "Toggle hotkey monitoring started for {} combination(s)",
                    auto_toggle_hotkeys.len()
                );
            }

            // Start PTT controller to handle toggle events (it handles both PTT and toggle)
            if let Err(e) = ptt_controller::start_ptt_controller() {
                warn!("Failed to start PTT controller for toggle handling: {}", e);
            }
        }

        // Set auto mode active for hotkey backend
        hotkey::set_auto_mode_active(true);
        {
            let state_arc = get_service_state();
            let mut state = state_arc.lock().await;
            state.auto_mode_active = true;
        }

        // Get sample rate from backend
        let sample_rate = platform::get_backend()
            .map(|b| b.sample_rate())
            .unwrap_or(48000);

        // Initialize transcribe state
        {
            let transcribe_state = get_transcribe_state();
            let mut transcribe = transcribe_state.lock().unwrap();
            transcribe.init_for_capture(sample_rate, 2);
            transcribe.activate();
        }

        // Start capture
        if let Some(backend) = platform::get_backend() {
            backend.set_aec_enabled(aec_enabled);
            backend.set_recording_mode(recording_mode);

            backend.start_capture_sources(source1_id, source2_id)?;
        } else {
            return Err("Audio backend not available".to_string());
        }

        // Start audio processing loop
        if !is_audio_loop_active() {
            let queue = get_transcription_queue();
            let transcribe_state = get_transcribe_state();
            start_audio_loop(queue, transcribe_state)?;
        }

        // Update state
        let state_arc = get_service_state();
        let mut state = state_arc.lock().await;
        state.transcribe_status.capturing = true;
        state.transcribe_status.error = None;

        info!("Audio capture started (Automatic mode)");

        // Broadcast event
        broadcast_event(Response::Event {
            event: EventType::CaptureStateChanged {
                capturing: true,
                error: None,
            },
        });

        Ok(())
    }
}

/// Stop audio capture.
async fn stop_capture() {
    // Stop PTT controller if running
    ptt_controller::stop_ptt_controller();

    // Stop hotkey monitoring
    hotkey::stop_hotkey();

    // Stop audio processing loop
    stop_audio_loop();

    // Finalize transcribe state
    {
        let transcribe_state = get_transcribe_state();
        let mut transcribe = transcribe_state.lock().unwrap();
        transcribe.finalize();
        transcribe.deactivate();
    }

    // Stop capture
    if let Some(backend) = platform::get_backend() {
        let _ = backend.stop_capture();
    }

    // Update state
    let state_arc = get_service_state();
    let mut state = state_arc.lock().await;
    state.transcribe_status.capturing = false;
    state.transcribe_status.in_speech = false;

    info!("Audio capture stopped");
}

/// Handle an IPC request and return a response.
pub async fn handle_request(request: Request) -> Response {
    // Validate request
    if let Err(e) = request.validate() {
        return Response::error(e);
    }

    match request {
        Request::Ping => Response::Pong,

        Request::GetRuntimeMode => {
            let state_arc = get_service_state();
            let state = state_arc.lock().await;
            Response::RuntimeMode {
                mode: state.runtime_mode.as_str().to_string(),
            }
        }

        Request::ListDevices { source_type } => {
            let mut devices = Vec::new();

            if let Some(backend) = platform::get_backend() {
                // Get input devices
                if source_type.is_none()
                    || matches!(
                        source_type,
                        Some(flowstt_common::AudioSourceType::Input)
                            | Some(flowstt_common::AudioSourceType::Mixed)
                    )
                {
                    devices.extend(backend.list_input_devices());
                }

                // Get system devices
                if source_type.is_none()
                    || matches!(
                        source_type,
                        Some(flowstt_common::AudioSourceType::System)
                            | Some(flowstt_common::AudioSourceType::Mixed)
                    )
                {
                    devices.extend(backend.list_system_devices());
                }
            }

            Response::Devices { devices }
        }

        Request::SetSources {
            source1_id,
            source2_id,
        } => {
            let state_arc = get_service_state();

            // Update source configuration and check if we should capture.
            // In PTT mode the hotkey backend and controller run even while
            // `capturing` is false (audio only flows while the key is held),
            // so we must also check if the PTT controller is active.
            let (was_active, should_capture) = {
                let mut state = state_arc.lock().await;
                let was = state.transcribe_status.capturing
                    || (state.transcription_mode == TranscriptionMode::PushToTalk
                        && ptt_controller::is_ptt_controller_running());
                state.source1_id = source1_id.clone();
                state.source2_id = source2_id.clone();
                (was, state.should_capture())
            };

            info!(
                "Audio sources changed: source1={:?}, source2={:?}",
                source1_id, source2_id
            );

            // Stop current capture / PTT monitoring if running
            if was_active {
                stop_capture().await;
            }

            // Start capture if app is ready and primary source is configured
            if should_capture {
                match start_capture().await {
                    Ok(()) => Response::Ok,
                    Err(e) => {
                        // Update error state
                        let mut state = state_arc.lock().await;
                        state.transcribe_status.error = Some(e.clone());

                        // Broadcast error
                        broadcast_event(Response::Event {
                            event: EventType::CaptureStateChanged {
                                capturing: false,
                                error: Some(e.clone()),
                            },
                        });

                        Response::error(e)
                    }
                }
            } else {
                // Not ready or no primary source - stay in ready state
                broadcast_event(Response::Event {
                    event: EventType::CaptureStateChanged {
                        capturing: false,
                        error: None,
                    },
                });
                Response::Ok
            }
        }

        Request::SetAecEnabled { enabled } => {
            let state_arc = get_service_state();
            let mut state = state_arc.lock().await;
            state.aec_enabled = enabled;

            // Apply to backend if capturing
            if state.transcribe_status.capturing {
                if let Some(backend) = platform::get_backend() {
                    backend.set_aec_enabled(enabled);
                }
            }

            info!("AEC enabled: {}", enabled);
            Response::Ok
        }

        Request::SetRecordingMode { mode } => {
            let state_arc = get_service_state();
            let mut state = state_arc.lock().await;
            state.recording_mode = mode;

            // Apply to backend if capturing
            if state.transcribe_status.capturing {
                if let Some(backend) = platform::get_backend() {
                    backend.set_recording_mode(mode);
                }
            }

            info!("Recording mode: {:?}", mode);
            Response::Ok
        }

        Request::GetStatus => {
            let state_arc = get_service_state();
            let state = state_arc.lock().await;

            // Update in_speech and queue_depth from transcribe state
            let mut status = state.transcribe_status.clone();
            if status.capturing {
                if let Ok(transcribe) = get_transcribe_state().try_lock() {
                    status.in_speech = transcribe.in_speech;
                }
                status.queue_depth = get_transcription_queue().queue_depth();
            }

            // Include current configuration in status
            status.source1_id = state.source1_id.clone();
            status.source2_id = state.source2_id.clone();
            status.transcription_mode = state.transcription_mode;

            Response::Status(status)
        }

        Request::GetConfig => {
            let state_arc = get_service_state();
            let state = state_arc.lock().await;

            let config = crate::config::Config::load();
            Response::ConfigValues(ConfigValues {
                transcription_mode: state.transcription_mode,
                ptt_hotkeys: state.ptt_hotkeys.clone(),
                auto_toggle_hotkeys: state.auto_toggle_hotkeys.clone(),
                auto_paste_enabled: config.auto_paste_enabled,
                auto_paste_delay_ms: config.auto_paste_delay_ms,
            })
        }

        Request::SubscribeEvents => {
            // Actual subscription is handled in the server
            Response::Subscribed
        }

        Request::GetModelStatus => {
            let transcriber = Transcriber::new();
            Response::ModelStatus(ModelStatus {
                available: transcriber.is_model_available(),
                path: transcriber.get_model_path().to_string_lossy().to_string(),
            })
        }

        Request::DownloadModel => {
            let transcriber = Transcriber::new();
            let model_path = transcriber.get_model_path().clone();

            if model_path.exists() {
                return Response::error("Model already downloaded");
            }

            // Download in background with streaming progress
            let path_clone = model_path.clone();
            tokio::spawn(async move {
                let result = download_model(&path_clone, |percent| {
                    broadcast_event(Response::Event {
                        event: EventType::ModelDownloadProgress { percent },
                    });
                })
                .await;

                match result {
                    Ok(()) => {
                        broadcast_event(Response::Event {
                            event: EventType::ModelDownloadComplete { success: true },
                        });
                    }
                    Err(e) => {
                        tracing::error!("Model download failed: {}", e);
                        broadcast_event(Response::Event {
                            event: EventType::ModelDownloadComplete { success: false },
                        });
                    }
                }
            });

            Response::Ok
        }

        Request::SetTranscriptionMode { mode } => {
            let state_arc = get_service_state();

            let (old_mode, is_ready, ptt_hotkeys) = {
                let mut state = state_arc.lock().await;
                let old_mode = state.transcription_mode;
                state.transcription_mode = mode;
                (
                    old_mode,
                    state.has_primary_source(),
                    state.ptt_hotkeys.clone(),
                )
            };

            info!(
                "Transcription mode change requested: {:?} -> {:?} (ready={})",
                old_mode, mode, is_ready
            );

            // If mode changed and system is ready, restart capture with new mode
            if old_mode != mode && is_ready {
                // Stop current capture
                stop_capture().await;

                // Restart with new mode
                if let Err(e) = start_capture().await {
                    warn!("Failed to restart capture after mode change: {}", e);
                }
            }

            // Save configuration to disk (load first to preserve other fields)
            let mut config = crate::config::Config::load();
            config.transcription_mode = mode;
            config.ptt_hotkeys = ptt_hotkeys;
            if let Err(e) = crate::config::save_config(&config) {
                warn!("Failed to save config: {}", e);
            }

            info!("Transcription mode set to {:?}", mode);

            // Broadcast mode change event
            broadcast_event(Response::Event {
                event: EventType::TranscriptionModeChanged { mode },
            });

            Response::Ok
        }

        Request::SetPushToTalkHotkeys { hotkeys } => {
            let state_arc = get_service_state();
            let (old_hotkeys, old_toggle, transcription_mode, is_ptt_monitoring) = {
                let mut state = state_arc.lock().await;
                let old_hotkeys = state.ptt_hotkeys.clone();
                let old_toggle = state.auto_toggle_hotkeys.clone();
                state.ptt_hotkeys = hotkeys.clone();
                // The hotkey backend runs whenever the PTT controller is
                // active, regardless of whether audio is currently capturing
                // (audio only flows while the key is held).
                let is_ptt_monitoring =
                    state.transcription_mode == TranscriptionMode::PushToTalk
                        && ptt_controller::is_ptt_controller_running();
                (old_hotkeys, old_toggle, state.transcription_mode, is_ptt_monitoring)
            };

            info!(
                "PTT hotkeys change requested: {} -> {} combinations (monitoring={})",
                old_hotkeys.len(),
                hotkeys.len(),
                is_ptt_monitoring
            );

            // If PTT monitoring is active, restart hotkey with new combinations
            if is_ptt_monitoring {
                hotkey::stop_hotkey();
                if let Err(e) = hotkey::start_hotkey(hotkeys.clone(), old_toggle.clone()) {
                    // Revert on failure
                    warn!("Failed to start hotkey with new combinations: {}", e);
                    let mut state = state_arc.lock().await;
                    state.ptt_hotkeys = old_hotkeys.clone();
                    let _ = hotkey::start_hotkey(old_hotkeys, old_toggle);
                    return Response::error(format!("Failed to set hotkeys: {}", e));
                }
            }

            // Save configuration to disk (load first to preserve other fields)
            let mut config = crate::config::Config::load();
            config.transcription_mode = transcription_mode;
            config.ptt_hotkeys = hotkeys;
            if let Err(e) = crate::config::save_config(&config) {
                warn!("Failed to save config: {}", e);
            }

            info!("PTT hotkeys updated");
            Response::Ok
        }

        Request::GetPttStatus => {
            let state_arc = get_service_state();
            let state = state_arc.lock().await;

            let available = hotkey::is_hotkey_available();
            let error = if !available {
                hotkey::hotkey_unavailable_reason()
            } else {
                None
            };

            Response::PttStatus(PttStatus {
                mode: state.transcription_mode,
                hotkeys: state.ptt_hotkeys.clone(),
                auto_toggle_hotkeys: state.auto_toggle_hotkeys.clone(),
                auto_mode_active: state.auto_mode_active,
                is_active: state.is_ptt_active,
                available,
                error,
                accessibility_permission_granted: hotkey::check_accessibility_permission(),
            })
        }

        Request::SetAutoToggleHotkeys { hotkeys } => {
            let state_arc = get_service_state();
            let (ptt_hotkeys, _transcription_mode, is_ptt_monitoring) = {
                let mut state = state_arc.lock().await;
                let _old_toggle = state.auto_toggle_hotkeys.clone();
                state.auto_toggle_hotkeys = hotkeys.clone();
                let is_ptt_monitoring =
                    state.transcription_mode == TranscriptionMode::PushToTalk
                        && ptt_controller::is_ptt_controller_running();
                (state.ptt_hotkeys.clone(), state.transcription_mode, is_ptt_monitoring)
            };

            info!("Auto-toggle hotkeys set: {} combination(s)", hotkeys.len());

            // If PTT monitoring is active, restart hotkey backend with new toggle hotkeys
            if is_ptt_monitoring {
                hotkey::stop_hotkey();
                if let Err(e) = hotkey::start_hotkey(ptt_hotkeys, hotkeys.clone()) {
                    warn!("Failed to restart hotkey with new toggle: {}", e);
                }
            }

            // Save config
            let mut config = crate::config::Config::load();
            config.auto_toggle_hotkeys = hotkeys;
            if let Err(e) = crate::config::save_config(&config) {
                warn!("Failed to save config: {}", e);
            }

            Response::Ok
        }

        Request::GetAutoToggleHotkeys => {
            let state_arc = get_service_state();
            let state = state_arc.lock().await;
            Response::ConfigValues(ConfigValues {
                transcription_mode: state.transcription_mode,
                ptt_hotkeys: state.ptt_hotkeys.clone(),
                auto_toggle_hotkeys: state.auto_toggle_hotkeys.clone(),
                auto_paste_enabled: true,
                auto_paste_delay_ms: 50,
            })
        }

        Request::ToggleAutoMode => {
            let state_arc = get_service_state();
            let (current_mode, auto_mode_active, _ptt_hotkeys, _toggle_hotkeys) = {
                let state = state_arc.lock().await;
                (
                    state.transcription_mode,
                    state.auto_mode_active,
                    state.ptt_hotkeys.clone(),
                    state.auto_toggle_hotkeys.clone(),
                )
            };

            let (new_mode, new_auto_active) = if auto_mode_active {
                (TranscriptionMode::PushToTalk, false)
            } else {
                (TranscriptionMode::Automatic, true)
            };

            info!("ToggleAutoMode: {:?} -> {:?}", current_mode, new_mode);

            // If currently recording in PTT mode, finalize first
            if ptt_controller::is_ptt_active() {
                // Signal to stop PTT recording
                // The toggle handler will handle this
            }

            // Update state
            {
                let mut state = state_arc.lock().await;
                state.transcription_mode = new_mode;
                state.auto_mode_active = new_auto_active;
                state.is_ptt_active = false;
                hotkey::set_auto_mode_active(new_auto_active);
            }

            // Save config
            let mut config = crate::config::Config::load();
            config.transcription_mode = new_mode;
            if let Err(e) = crate::config::save_config(&config) {
                warn!("Failed to save config: {}", e);
            }

            // Broadcast event
            broadcast_event(Response::Event {
                event: EventType::AutoModeToggled { mode: new_mode },
            });

            Response::Ok
        }

        Request::GetCudaStatus => {
            // Check build-time GPU support
            // Windows always uses CUDA binaries (auto CPU fallback when no GPU)
            #[cfg(target_os = "windows")]
            let build_enabled = true;
            // macOS uses Metal acceleration via prebuilt whisper.cpp framework
            #[cfg(target_os = "macos")]
            let build_enabled = true;
            // Linux requires cuda feature flag (CUDA toolkit at build time)
            #[cfg(all(target_os = "linux", feature = "cuda"))]
            let build_enabled = true;
            #[cfg(all(target_os = "linux", not(feature = "cuda")))]
            let build_enabled = false;

            // Get system info from whisper.cpp
            let (runtime_available, system_info) =
                match crate::transcription::whisper_ffi::get_system_info() {
                    Ok(info) => {
                        let gpu_available = info.contains("CUDA : ARCHS")
                            || info.contains("METAL = 1")
                            || info.contains("VULKAN = 1");
                        (gpu_available, info)
                    }
                    Err(e) => (false, format!("Error: {}", e)),
                };

            Response::CudaStatus(CudaStatus {
                build_enabled,
                runtime_available,
                system_info,
            })
        }

        Request::SetAutoPaste { enabled } => {
            // Load current config, update the auto-paste setting, and save
            let mut config = crate::config::Config::load();
            config.auto_paste_enabled = enabled;
            if let Err(e) = crate::config::save_config(&config) {
                warn!("Failed to save config: {}", e);
            }

            info!("Auto-paste set to {}", enabled);
            Response::Ok
        }

        Request::GetHistory => {
            let history = crate::history::get_history();
            let h = history.lock().unwrap();
            let entries: Vec<flowstt_common::HistoryEntry> = h
                .get_entries()
                .iter()
                .map(|e| flowstt_common::HistoryEntry {
                    id: e.id.clone(),
                    text: e.text.clone(),
                    timestamp: e.timestamp.clone(),
                    wav_path: e.wav_path.clone(),
                })
                .collect();
            Response::History { entries }
        }

        Request::DeleteHistoryEntry { id } => {
            let history = crate::history::get_history();
            let deleted = {
                let mut h = history.lock().unwrap();
                h.delete_entry(&id)
            };
            if deleted {
                info!("Deleted history entry: {}", id);
                // Broadcast deletion event to all subscribed clients
                broadcast_event(Response::Event {
                    event: EventType::HistoryEntryDeleted { id },
                });
                Response::Ok
            } else {
                Response::error(format!("History entry not found: {}", id))
            }
        }

        Request::TestAudioDevice { device_id } => {
            // Stop any existing test capture (handles device switching)
            crate::test_capture::stop_test_capture();

            // Stop the main audio loop so it doesn't race on try_recv().
            // The audio backend is a singleton with a single mpsc channel;
            // only one consumer can poll it at a time.
            if is_audio_loop_active() {
                stop_audio_loop();
                if let Some(backend) = platform::get_backend() {
                    let _ = backend.stop_capture();
                }
            }

            match crate::test_capture::start_test_capture(device_id) {
                Ok(()) => Response::Ok,
                Err(e) => Response::error(e),
            }
        }

        Request::StopTestAudioDevice => {
            crate::test_capture::stop_test_capture();
            Response::Ok
        }

        Request::CheckAccessibilityPermission => {
            let granted = hotkey::check_accessibility_permission();
            info!("[Hotkey] Accessibility permission check: granted={}", granted);
            Response::AccessibilityPermission { granted }
        }

        Request::RequestAccessibilityPermission => {
            let granted = hotkey::request_accessibility_permission();
            info!("[Hotkey] Accessibility permission requested: granted={}", granted);
            Response::AccessibilityPermission { granted }
        }

        Request::Shutdown => {
            info!("Shutdown requested via IPC");

            // Stop capture
            stop_capture().await;

            // Stop transcription worker
            get_transcription_queue().stop_worker();

            // Broadcast shutdown event
            broadcast_event(Response::Event {
                event: EventType::Shutdown,
            });

            crate::request_shutdown();
            Response::Ok
        }
    }
}
