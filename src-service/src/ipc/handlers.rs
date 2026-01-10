//! IPC request handlers.

use flowstt_common::ipc::{EventType, Request, Response};
use flowstt_common::{CudaStatus, ModelStatus};
use std::sync::Arc;
use tracing::info;

use super::server::broadcast_event;
use crate::platform;
use crate::state::get_service_state;
use crate::transcription::{download_model, TranscribeState, Transcriber, TranscriptionQueue};
use crate::{
    is_audio_loop_active, start_audio_loop, stop_audio_loop, TranscriptionEventBroadcaster,
};

/// Global transcription queue
static TRANSCRIPTION_QUEUE: std::sync::OnceLock<Arc<TranscriptionQueue>> =
    std::sync::OnceLock::new();

fn get_transcription_queue() -> Arc<TranscriptionQueue> {
    TRANSCRIPTION_QUEUE
        .get_or_init(|| Arc::new(TranscriptionQueue::new()))
        .clone()
}

/// Global transcribe state
static TRANSCRIBE_STATE: std::sync::OnceLock<Arc<std::sync::Mutex<TranscribeState>>> =
    std::sync::OnceLock::new();

fn get_transcribe_state() -> Arc<std::sync::Mutex<TranscribeState>> {
    TRANSCRIBE_STATE
        .get_or_init(|| {
            let queue = get_transcription_queue();
            Arc::new(std::sync::Mutex::new(TranscribeState::new(queue)))
        })
        .clone()
}

/// Handle an IPC request and return a response.
pub async fn handle_request(request: Request) -> Response {
    // Validate request
    if let Err(e) = request.validate() {
        return Response::error(e);
    }

    match request {
        Request::Ping => Response::Pong,

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

        Request::StartTranscribe {
            source1_id,
            source2_id,
            aec_enabled,
            mode,
            transcription_enabled,
        } => {
            let state_arc = get_service_state();
            let mut state = state_arc.lock().await;

            if state.transcribe_status.active {
                return Response::error("Transcription already active");
            }

            // Update state
            state.source1_id = source1_id.clone();
            state.source2_id = source2_id.clone();
            state.aec_enabled = aec_enabled;
            state.recording_mode = mode;

            // Get sample rate from backend
            let sample_rate = platform::get_backend()
                .map(|b| b.sample_rate())
                .unwrap_or(48000);

            // Initialize transcribe state (only activate if transcription is enabled)
            {
                let transcribe_state = get_transcribe_state();
                let mut transcribe = transcribe_state.lock().unwrap();
                transcribe.init_for_capture(sample_rate, 2);
                if transcription_enabled {
                    transcribe.activate();
                }
            }

            // Only start transcription worker if transcription is enabled
            if transcription_enabled {
                // Set up transcription queue callback
                let queue = get_transcription_queue();
                queue.set_callback(Arc::new(TranscriptionEventBroadcaster));

                // Start transcription worker
                let transcriber = Transcriber::new();
                let model_path = transcriber.get_model_path().clone();
                queue.start_worker(model_path);
            }

            // Start capture
            if let Some(backend) = platform::get_backend() {
                backend.set_aec_enabled(aec_enabled);
                backend.set_recording_mode(mode);

                if let Err(e) = backend.start_capture_sources(source1_id, source2_id) {
                    return Response::error(e);
                }
            } else {
                return Response::error("Audio backend not available");
            }

            // Start audio processing loop (needed for visualization even without transcription)
            if !is_audio_loop_active() {
                let queue = get_transcription_queue();
                let transcribe_state = get_transcribe_state();
                if let Err(e) = start_audio_loop(queue, transcribe_state) {
                    return Response::error(e);
                }
            }

            state.transcribe_status.active = true;
            info!(
                "{}",
                if transcription_enabled {
                    "Transcription started"
                } else {
                    "Monitoring started (transcription disabled)"
                }
            );

            // Broadcast event
            broadcast_event(Response::Event {
                event: EventType::TranscribeStarted,
            });

            Response::Ok
        }

        Request::StopTranscribe => {
            let state_arc = get_service_state();
            let mut state = state_arc.lock().await;

            if !state.transcribe_status.active {
                return Response::error("Transcription not active");
            }

            // Stop audio processing loop
            stop_audio_loop();

            // Finalize transcribe state
            {
                let transcribe_state = get_transcribe_state();
                let mut transcribe = transcribe_state.lock().unwrap();
                transcribe.finalize();
                transcribe.deactivate();
            }

            // Stop transcription worker (will drain queue)
            get_transcription_queue().stop_worker();

            // Stop capture
            if let Some(backend) = platform::get_backend() {
                if let Err(e) = backend.stop_capture() {
                    return Response::error(e);
                }
            }

            state.transcribe_status.active = false;
            state.transcribe_status.in_speech = false;
            info!("Transcription stopped");

            // Broadcast event
            broadcast_event(Response::Event {
                event: EventType::TranscribeStopped,
            });

            Response::Ok
        }

        Request::GetStatus => {
            let state_arc = get_service_state();
            let state = state_arc.lock().await;

            // Update in_speech and queue_depth from transcribe state
            let mut status = state.transcribe_status.clone();
            if status.active {
                if let Ok(transcribe) = get_transcribe_state().try_lock() {
                    status.in_speech = transcribe.in_speech;
                }
                status.queue_depth = get_transcription_queue().queue_depth();
            }

            Response::Status(status)
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

            // Download in background
            let path_clone = model_path.clone();
            tokio::task::spawn_blocking(move || {
                // Broadcast progress (simplified - just start/end)
                broadcast_event(Response::Event {
                    event: EventType::ModelDownloadProgress { percent: 0 },
                });

                let result = download_model(&path_clone);

                match result {
                    Ok(()) => {
                        broadcast_event(Response::Event {
                            event: EventType::ModelDownloadProgress { percent: 100 },
                        });
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

        Request::GetCudaStatus => {
            // Check build-time CUDA support
            #[cfg(all(any(target_os = "linux", target_os = "windows"), feature = "cuda"))]
            let build_enabled = true;
            #[cfg(not(all(any(target_os = "linux", target_os = "windows"), feature = "cuda")))]
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

        Request::Shutdown => {
            info!("Shutdown requested via IPC");

            // Stop audio loop and transcription
            stop_audio_loop();
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
