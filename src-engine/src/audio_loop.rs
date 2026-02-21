//! Audio processing loop for the service.
//!
//! This module connects the platform audio backend to the speech detection
//! and transcription systems. In Automatic mode, uses VAD to trigger transcription.
//! In PTT mode, the PTT controller manages transcription triggers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use flowstt_common::ipc::{EventType, Response};
use flowstt_common::{TranscriptionResult, VisualizationData};
use tracing::{debug, error, info};

use crate::ipc::broadcast_event;
use crate::platform;
use crate::processor::{
    SpeechDetector, SpeechEventCallback, SpeechEventPayload, SpeechStateChange,
    VisualizationCallback, VisualizationPayload, VisualizationProcessor, WordBreakEvent,
    WordBreakPayload,
};
use crate::transcription::{TranscribeState, TranscriptionCallback, TranscriptionQueue};

/// Global audio processing thread control
static AUDIO_LOOP_ACTIVE: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

fn get_loop_active() -> Arc<AtomicBool> {
    AUDIO_LOOP_ACTIVE
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Check if the audio loop is running
pub fn is_audio_loop_active() -> bool {
    get_loop_active().load(Ordering::SeqCst)
}

/// Start the audio processing loop
pub fn start_audio_loop(
    _transcription_queue: Arc<TranscriptionQueue>,
    transcribe_state: Arc<std::sync::Mutex<TranscribeState>>,
) -> Result<(), String> {
    if is_audio_loop_active() {
        return Err("Audio loop already running".into());
    }

    let loop_active = get_loop_active();
    loop_active.store(true, Ordering::SeqCst);

    // Get sample rate from backend
    let sample_rate = platform::get_backend()
        .map(|b| b.sample_rate())
        .unwrap_or(48000);

    thread::spawn(move || {
        tracing::info!("[AudioLoop] Starting audio processing loop");

        // Create speech detector
        let mut speech_detector = SpeechDetector::new(sample_rate);
        speech_detector.set_callback(Arc::new(SpeechEventBroadcaster));

        // Create visualization processor
        let mut viz_processor = VisualizationProcessor::new(sample_rate, 256);
        viz_processor.set_callback(Arc::new(VisualizationBroadcaster));

        let loop_active = get_loop_active();

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
                // Convert to mono for processing
                let mono_samples = convert_to_mono(&data.samples, data.channels as usize);

                // Process through speech detector (always run for visualization)
                speech_detector.process(&mono_samples);

                // Get speech metrics for visualization
                let speech_metrics = speech_detector.get_metrics();
                viz_processor.set_speech_metrics(speech_metrics);

                // Process visualization
                viz_processor.process(&mono_samples);

                // Handle speech state changes for transcribe mode
                let state_change = speech_detector.take_state_change();
                let word_break = speech_detector.take_word_break_event();

                // Update transcribe state if active
                // Note: In Automatic mode, VAD triggers segments
                // In PTT mode, PTT controller triggers segments (not audio_loop)
                if let Ok(mut transcribe) = transcribe_state.try_lock() {
                    if transcribe.is_active {
                        // Write samples to ring buffer
                        transcribe.process_samples(&data.samples);

                        // Use speech detection events to trigger segments
                        match state_change {
                            SpeechStateChange::Started { lookback_samples } => {
                                transcribe.on_speech_started(lookback_samples);

                                // Broadcast speech started event
                                broadcast_event(Response::Event {
                                    event: EventType::SpeechStarted,
                                });
                            }
                            SpeechStateChange::Ended { duration_ms } => {
                                transcribe.on_speech_ended();

                                // Broadcast speech ended event
                                broadcast_event(Response::Event {
                                    event: EventType::SpeechEnded { duration_ms },
                                });
                            }
                            SpeechStateChange::None => {}
                        }

                        // Handle word breaks for timed segment submission
                        if let Some(WordBreakEvent {
                            offset_ms,
                            gap_duration_ms,
                        }) = word_break
                        {
                            transcribe.on_word_break(offset_ms, gap_duration_ms);
                        }
                    }
                }
            } else {
                // No data available, sleep briefly
                thread::sleep(Duration::from_millis(1));
            }
        }

        tracing::info!("[AudioLoop] Audio processing loop stopped");
    });

    Ok(())
}

/// Stop the audio processing loop
pub fn stop_audio_loop() {
    get_loop_active().store(false, Ordering::SeqCst);
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

/// Broadcaster for speech events
struct SpeechEventBroadcaster;

impl SpeechEventCallback for SpeechEventBroadcaster {
    fn on_speech_started(&self, _payload: SpeechEventPayload) {
        // Handled in the main loop
    }

    fn on_speech_ended(&self, _payload: SpeechEventPayload) {
        // Handled in the main loop
    }

    fn on_word_break(&self, _payload: WordBreakPayload) {
        // Handled in the main loop
    }
}

/// Broadcaster for visualization events
struct VisualizationBroadcaster;

impl VisualizationCallback for VisualizationBroadcaster {
    fn on_visualization_data(&self, payload: VisualizationPayload) {
        // Convert processor payload to common VisualizationData
        let data = VisualizationData {
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

/// Callback for transcription events - broadcasts to IPC clients
pub struct TranscriptionEventBroadcaster;

impl TranscriptionCallback for TranscriptionEventBroadcaster {
    fn on_transcription_started(&self) {
        debug!("[Transcription] Started");
    }

    fn on_transcription_complete(&self, text: String, wav_path: Option<String>) {
        info!("[Transcription] Complete: {}", text);

        // Add to persistent history and get the enriched entry
        let history = crate::history::get_history();
        let entry = {
            let mut h = history.lock().unwrap();
            h.add_entry(text.clone(), wav_path)
        };

        broadcast_event(Response::Event {
            event: EventType::TranscriptionComplete(TranscriptionResult {
                id: Some(entry.id),
                text: entry.text.clone(),
                timestamp: Some(entry.timestamp),
                audio_path: entry.wav_path,
            }),
        });

        // Copy to clipboard and optionally paste into the foreground app.
        // Config is loaded from disk so runtime changes take effect immediately.
        let config = crate::config::Config::load();
        crate::clipboard::copy_and_paste(
            &entry.text,
            config.auto_paste_enabled,
            config.auto_paste_delay_ms,
        );
    }

    fn on_transcription_error(&self, error: String) {
        error!("[Transcription] Error: {}", error);
    }

    fn on_transcription_finished(&self) {
        debug!("[Transcription] Finished");
    }

    fn on_queue_update(&self, depth: usize) {
        debug!("[Transcription] Queue depth: {}", depth);
    }
}
