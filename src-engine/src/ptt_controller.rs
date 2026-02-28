//! Push-to-Talk Controller
//!
//! This module manages the PTT lifecycle:
//! - In PTT mode, audio capture is only active while the hotkey is held
//! - Polls for hotkey events independently of audio loop
//! - Starts/stops audio capture on key press/release
//! - Handles toggle hotkey for switching between modes

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use flowstt_common::ipc::{EventType, Response};
use flowstt_common::TranscriptionMode;
use tracing::{debug, error, info};

use crate::audio_loop::{self, is_audio_loop_active};
use crate::hotkey::{self, HotkeyEvent};
use crate::ipc::broadcast_event;
use crate::ipc::handlers::{get_transcribe_state, get_transcription_queue};
use crate::platform;
use crate::processor::{VisualizationCallback, VisualizationPayload, VisualizationProcessor};
use crate::state::get_service_state;

/// Global PTT controller state
static PTT_ACTIVE: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
static PTT_THREAD_RUNNING: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

fn get_ptt_active() -> Arc<AtomicBool> {
    PTT_ACTIVE
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn get_ptt_thread_running() -> Arc<AtomicBool> {
    PTT_THREAD_RUNNING
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Check if PTT controller thread is running
pub fn is_ptt_controller_running() -> bool {
    get_ptt_thread_running().load(Ordering::SeqCst)
}

/// Start the PTT controller thread.
/// This monitors for hotkey events and controls audio capture accordingly.
pub fn start_ptt_controller() -> Result<(), String> {
    if get_ptt_thread_running().load(Ordering::SeqCst) {
        return Ok(()); // Already running
    }

    info!("[PTT Controller] Starting...");

    get_ptt_thread_running().store(true, Ordering::SeqCst);
    get_ptt_active().store(false, Ordering::SeqCst);

    thread::spawn(move || {
        ptt_controller_loop();
    });

    Ok(())
}

/// Stop the PTT controller thread.
pub fn stop_ptt_controller() {
    if !get_ptt_thread_running().load(Ordering::SeqCst) {
        return;
    }

    info!("[PTT Controller] Stopping...");
    get_ptt_thread_running().store(false, Ordering::SeqCst);

    // If PTT was active, stop capture
    if get_ptt_active().load(Ordering::SeqCst) {
        get_ptt_active().store(false, Ordering::SeqCst);
        stop_ptt_capture();
    }
}

/// Main PTT controller loop
fn ptt_controller_loop() {
    info!("[PTT] Controller polling for hotkey events...");

    while get_ptt_thread_running().load(Ordering::SeqCst) {
        // Check if we should stop
        if crate::is_shutdown_requested() {
            break;
        }

        // Check for hotkey events
        if let Some(event) = hotkey::try_recv_hotkey() {
            match event {
                HotkeyEvent::PttPressed => {
                    handle_ptt_pressed();
                }
                HotkeyEvent::PttReleased => {
                    handle_ptt_released();
                }
                HotkeyEvent::TogglePressed => {
                    handle_toggle_pressed();
                }
            }
        }

        // Sleep briefly to avoid busy-waiting
        thread::sleep(Duration::from_millis(5));
    }

    info!("[PTT] Controller stopped");
    get_ptt_thread_running().store(false, Ordering::SeqCst);
}

/// Handle PTT key press - start audio capture.
/// Public within the crate so the test mode orchestrator can trigger PTT programmatically.
pub(crate) fn handle_ptt_pressed() {
    if get_ptt_active().load(Ordering::SeqCst) {
        return;
    }

    info!("[PTT] Recording STARTED");
    get_ptt_active().store(true, Ordering::SeqCst);

    // Update state
    {
        let state_arc = get_service_state();
        let mut state = futures::executor::block_on(state_arc.lock());
        state.is_ptt_active = true;
    }

    // Broadcast PTT pressed event
    broadcast_event(Response::Event {
        event: EventType::PttPressed,
    });

    // Start capture
    if let Err(e) = start_ptt_capture() {
        error!("[PTT] Failed to start recording: {}", e);
        get_ptt_active().store(false, Ordering::SeqCst);

        broadcast_event(Response::Event {
            event: EventType::CaptureStateChanged {
                capturing: false,
                error: Some(e),
            },
        });
    } else {
        // Broadcast speech started
        broadcast_event(Response::Event {
            event: EventType::SpeechStarted,
        });

        broadcast_event(Response::Event {
            event: EventType::CaptureStateChanged {
                capturing: true,
                error: None,
            },
        });
    }
}

/// Handle PTT key release - stop audio capture and submit segment.
/// Public within the crate so the test mode orchestrator can trigger PTT programmatically.
pub(crate) fn handle_ptt_released() {
    if !get_ptt_active().load(Ordering::SeqCst) {
        return;
    }

    info!("[PTT] Recording STOPPED - submitting for transcription");
    get_ptt_active().store(false, Ordering::SeqCst);

    // Update state
    {
        let state_arc = get_service_state();
        let mut state = futures::executor::block_on(state_arc.lock());
        state.is_ptt_active = false;
    }

    // Finalize current segment before stopping - this submits for transcription
    let transcribe_state = get_transcribe_state();
    if let Ok(mut transcribe) = transcribe_state.try_lock() {
        info!(
            "[PTT] Finalizing segment: in_speech={}, is_active={}",
            transcribe.in_speech, transcribe.is_active
        );
        if transcribe.in_speech {
            let segment = transcribe.on_speech_ended();
            if let Some(ref s) = segment {
                info!("[PTT] Extracted {} samples for transcription", s.len());
            } else {
                info!("[PTT] No segment extracted (empty or invalid)");
            }
        }
    } else {
        error!("[PTT] Failed to lock transcribe state for finalization");
    }

    // Stop capture
    stop_ptt_capture();

    // Broadcast events
    broadcast_event(Response::Event {
        event: EventType::PttReleased,
    });

    broadcast_event(Response::Event {
        event: EventType::SpeechEnded { duration_ms: 0 },
    });

    broadcast_event(Response::Event {
        event: EventType::CaptureStateChanged {
            capturing: false,
            error: None,
        },
    });
}

/// Handle toggle hotkey press - switch between Automatic and PTT modes
fn handle_toggle_pressed() {
    info!("[Toggle] Toggle hotkey pressed");

    let state_arc = get_service_state();
    let (
        current_mode,
        auto_mode_active,
        _ptt_hotkeys,
        _toggle_hotkeys,
        source1_id,
        source2_id,
        aec_enabled,
        recording_mode,
    ) = {
        let state = futures::executor::block_on(state_arc.lock());
        (
            state.transcription_mode,
            state.auto_mode_active,
            state.ptt_hotkeys.clone(),
            state.auto_toggle_hotkeys.clone(),
            state.source1_id.clone(),
            state.source2_id.clone(),
            state.aec_enabled,
            state.recording_mode,
        )
    };

    // Determine new mode
    let (new_mode, new_auto_active) = if auto_mode_active {
        // Currently in auto mode via toggle -> switch to PTT mode
        (TranscriptionMode::PushToTalk, false)
    } else {
        // Currently in PTT mode -> switch to auto mode
        (TranscriptionMode::Automatic, true)
    };

    info!(
        "[Toggle] Switching from {:?} to {:?}",
        current_mode, new_mode
    );

    // If currently recording in PTT mode, finalize the segment first
    if get_ptt_active().load(Ordering::SeqCst) {
        info!("[Toggle] Finalizing active PTT segment before mode switch");
        get_ptt_active().store(false, Ordering::SeqCst);

        let transcribe_state = get_transcribe_state();
        if let Ok(mut transcribe) = transcribe_state.try_lock() {
            if transcribe.in_speech {
                let _ = transcribe.on_speech_ended();
            }
        }
        stop_ptt_capture();
    }

    // Stop any existing audio loop and capture
    audio_loop::stop_audio_loop();
    if let Some(backend) = platform::get_backend() {
        let _ = backend.stop_capture();
    }

    // Update state
    {
        let state_arc = get_service_state();
        let mut state = futures::executor::block_on(state_arc.lock());
        state.transcription_mode = new_mode;
        state.auto_mode_active = new_auto_active;
        state.is_ptt_active = false;
        state.transcribe_status.capturing = false;
        state.transcribe_status.in_speech = false;

        // Update auto mode suppression in hotkey backend
        hotkey::set_auto_mode_active(new_auto_active);
    }

    // Save config
    let mut config = crate::config::Config::load();
    config.transcription_mode = new_mode;
    if let Err(e) = crate::config::save_config(&config) {
        error!("[Toggle] Failed to save config: {}", e);
    }

    // Broadcast toggle event
    broadcast_event(Response::Event {
        event: EventType::AutoModeToggled { mode: new_mode },
    });

    // Start capture for the new mode
    if new_mode == TranscriptionMode::Automatic {
        // Start continuous capture with VAD
        info!("[Toggle] Starting Automatic mode capture");

        let has_source = source1_id.is_some();
        if !has_source {
            error!("[Toggle] No audio source configured");
            broadcast_event(Response::Event {
                event: EventType::CaptureStateChanged {
                    capturing: false,
                    error: Some("No audio source configured".to_string()),
                },
            });
            return;
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

            if let Err(e) = backend.start_capture_sources(source1_id, source2_id) {
                error!("[Toggle] Failed to start capture: {}", e);
                broadcast_event(Response::Event {
                    event: EventType::CaptureStateChanged {
                        capturing: false,
                        error: Some(e),
                    },
                });
                return;
            }
        }

        // Start audio processing loop
        if !is_audio_loop_active() {
            let queue = get_transcription_queue();
            let transcribe_state = get_transcribe_state();
            if let Err(e) = audio_loop::start_audio_loop(queue, transcribe_state) {
                error!("[Toggle] Failed to start audio loop: {}", e);
            }
        }

        // Update state
        {
            let state_arc = get_service_state();
            let mut state = futures::executor::block_on(state_arc.lock());
            state.transcribe_status.capturing = true;
            state.transcribe_status.error = None;
        }

        broadcast_event(Response::Event {
            event: EventType::CaptureStateChanged {
                capturing: true,
                error: None,
            },
        });

        info!("[Toggle] Automatic mode capture started");
    } else {
        // PTT mode - just ready state, capture happens on hotkey press
        info!("[Toggle] PTT mode ready - waiting for hotkey press");

        broadcast_event(Response::Event {
            event: EventType::CaptureStateChanged {
                capturing: false,
                error: None,
            },
        });
    }

    info!("[Toggle] Mode switched to {:?}", new_mode);
}

/// Start audio capture for PTT session
fn start_ptt_capture() -> Result<(), String> {
    let state_arc = get_service_state();
    let (source1_id, source2_id, aec_enabled, recording_mode) = {
        let state = futures::executor::block_on(state_arc.lock());

        if !state.has_primary_source() {
            return Err("No primary audio source configured".to_string());
        }

        (
            state.source1_id.clone(),
            state.source2_id.clone(),
            state.aec_enabled,
            state.recording_mode,
        )
    };

    // Get sample rate from backend
    let sample_rate = platform::get_backend()
        .map(|b| b.sample_rate())
        .unwrap_or(48000);

    // Initialize transcribe state for PTT mode
    {
        let transcribe_state = get_transcribe_state();
        let mut transcribe = transcribe_state.lock().unwrap();
        transcribe.init_for_capture(sample_rate, 2);
        transcribe.set_ptt_mode(true); // Disable automatic segmentation
        transcribe.activate();
        // Immediately start speech segment (no lookback in PTT mode)
        transcribe.on_speech_started(0);
    }

    // Start capture
    if let Some(backend) = platform::get_backend() {
        backend.set_aec_enabled(aec_enabled);
        backend.set_recording_mode(recording_mode);

        backend.start_capture_sources(source1_id, source2_id)?;
    } else {
        return Err("Audio backend not available".to_string());
    }

    // Start PTT audio processing loop (simpler than the main audio loop - no VAD)
    start_ptt_audio_loop();

    // Update state
    {
        let mut state = futures::executor::block_on(state_arc.lock());
        state.transcribe_status.capturing = true;
        state.transcribe_status.error = None;
    }

    debug!("[PTT] Audio capture started");
    Ok(())
}

/// Stop audio capture for PTT session
fn stop_ptt_capture() {
    // Stop PTT audio processing loop
    stop_ptt_audio_loop();

    // Finalize transcribe state and disable PTT mode
    let transcribe_state = get_transcribe_state();
    if let Ok(mut transcribe) = transcribe_state.try_lock() {
        transcribe.finalize();
        transcribe.deactivate();
        transcribe.set_ptt_mode(false); // Restore automatic segmentation for next use
    }

    // Stop capture
    if let Some(backend) = platform::get_backend() {
        let _ = backend.stop_capture();
    }

    // Update state
    {
        let state_arc = get_service_state();
        let mut state = futures::executor::block_on(state_arc.lock());
        state.transcribe_status.capturing = false;
        state.transcribe_status.in_speech = false;
    }

    info!("[PTT Controller] Capture stopped");
}

/// Global PTT audio loop control
static PTT_AUDIO_LOOP_ACTIVE: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

fn get_ptt_audio_loop_active() -> Arc<AtomicBool> {
    PTT_AUDIO_LOOP_ACTIVE
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Start PTT audio processing loop (simplified - no VAD, just process audio)
fn start_ptt_audio_loop() {
    if get_ptt_audio_loop_active().load(Ordering::SeqCst) {
        return; // Already running
    }

    let loop_active = get_ptt_audio_loop_active();
    loop_active.store(true, Ordering::SeqCst);

    // Get sample rate from backend
    let sample_rate = platform::get_backend()
        .map(|b| b.sample_rate())
        .unwrap_or(48000);

    let transcribe_state = get_transcribe_state();

    thread::spawn(move || {
        debug!("[PTT AudioLoop] Starting PTT audio processing loop");

        // Create visualization processor
        let mut viz_processor = VisualizationProcessor::new(sample_rate, 256);
        viz_processor.set_callback(Arc::new(PttVisualizationBroadcaster));

        let loop_active = get_ptt_audio_loop_active();

        loop {
            // Check if we should stop
            if !loop_active.load(Ordering::SeqCst) {
                break;
            }

            // Check if service is shutting down
            if crate::is_shutdown_requested() {
                break;
            }

            // Try to receive audio from backend
            let audio_data = platform::get_backend().and_then(|b| b.try_recv());

            if let Some(data) = audio_data {
                // Convert to mono for visualization
                let mono_samples = convert_to_mono(&data.samples, data.channels as usize);

                // Process visualization
                viz_processor.process(&mono_samples);

                // Write audio to transcribe state (no VAD - PTT controller manages segments)
                if let Ok(mut transcribe) = transcribe_state.try_lock() {
                    if transcribe.is_active {
                        transcribe.process_samples(&data.samples);
                    }
                }
            } else {
                // No data available, sleep briefly
                thread::sleep(Duration::from_millis(1));
            }
        }

        debug!("[PTT AudioLoop] PTT audio processing loop stopped");
    });
}

/// Stop PTT audio processing loop
fn stop_ptt_audio_loop() {
    get_ptt_audio_loop_active().store(false, Ordering::SeqCst);
}

/// Convert multi-channel audio to mono
fn convert_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Broadcaster for PTT visualization events
struct PttVisualizationBroadcaster;

impl VisualizationCallback for PttVisualizationBroadcaster {
    fn on_visualization_data(&self, payload: VisualizationPayload) {
        // Convert processor payload to common VisualizationData
        let data = flowstt_common::VisualizationData {
            waveform: payload.waveform,
            spectrogram: payload
                .spectrogram
                .map(|s| flowstt_common::SpectrogramColumn { colors: s.colors }),
            speech_metrics: payload
                .speech_metrics
                .map(|m| flowstt_common::SpeechMetrics {
                    amplitude_db: m.amplitude_db,
                    zcr: m.zcr,
                    centroid_hz: m.centroid_hz,
                    is_speaking: m.is_speaking,
                    voiced_onset_pending: m.is_voiced_pending,
                    whisper_onset_pending: m.is_whisper_pending,
                    is_transient: m.is_transient,
                    is_lookback_speech: m.is_lookback_speech,
                    is_word_break: m.is_word_break,
                }),
        };
        broadcast_event(Response::Event {
            event: EventType::VisualizationData(data),
        });
    }
}

/// Check if PTT is currently active (key held)
#[allow(dead_code)]
pub fn is_ptt_active() -> bool {
    get_ptt_active().load(Ordering::SeqCst)
}
