//! Lightweight audio device test capture for the setup wizard.
//!
//! This module provides a simple way to start a temporary capture on a single
//! device and broadcast audio level updates without engaging the full
//! transcription pipeline. It is used by the setup wizard to show a live
//! audio level meter during device selection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use flowstt_common::ipc::{EventType, Response};

use crate::ipc::broadcast_event;
use crate::platform;

/// Global state for the active test capture.
static TEST_CAPTURE: std::sync::OnceLock<Mutex<Option<TestCaptureHandle>>> =
    std::sync::OnceLock::new();

fn get_test_capture() -> &'static Mutex<Option<TestCaptureHandle>> {
    TEST_CAPTURE.get_or_init(|| Mutex::new(None))
}

struct TestCaptureHandle {
    device_id: String,
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Stop any active test capture and wait for its thread to finish.
fn stop_and_join(current: &mut Option<TestCaptureHandle>) {
    if let Some(mut handle) = current.take() {
        handle.stop_flag.store(true, Ordering::Relaxed);
        if let Some(thread) = handle.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Start a test capture on the specified device.
///
/// If a test capture is already active on a different device, it is stopped
/// and joined first. If the same device is already being tested, this is a
/// no-op.
pub fn start_test_capture(device_id: String) -> Result<(), String> {
    let guard = get_test_capture();
    let mut current = guard.lock().unwrap();

    // If already testing this device, nothing to do
    if let Some(ref handle) = *current {
        if handle.device_id == device_id {
            return Ok(());
        }
    }

    // Stop the old capture and wait for its thread to exit so it releases
    // the audio backend before we start a new capture.
    stop_and_join(&mut current);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = stop_flag.clone();
    let dev_id = device_id.clone();

    let thread = std::thread::spawn(move || {
        if let Err(e) = run_test_capture(&dev_id, &flag_clone) {
            tracing::warn!("Test capture ended with error: {}", e);
        }
    });

    *current = Some(TestCaptureHandle {
        device_id,
        stop_flag,
        thread: Some(thread),
    });

    Ok(())
}

/// Stop any active test capture.
pub fn stop_test_capture() {
    let guard = get_test_capture();
    let mut current = guard.lock().unwrap();
    stop_and_join(&mut current);
}

/// Run the test capture loop. This blocks until the stop flag is set.
fn run_test_capture(device_id: &str, stop_flag: &AtomicBool) -> Result<(), String> {
    let backend = platform::get_backend().ok_or("Audio backend not available")?;

    // Start capture on the single device
    backend.start_capture_sources(Some(device_id.to_string()), None)?;

    let sample_rate = backend.sample_rate();
    // Target ~10 Hz updates: accumulate samples for ~100ms before computing level
    let samples_per_update = (sample_rate as usize) / 10;
    let mut accumulated = Vec::with_capacity(samples_per_update);

    while !stop_flag.load(Ordering::Relaxed) {
        if let Some(audio_data) = backend.try_recv() {
            // Convert to mono if multi-channel
            let mono: Vec<f32> = if audio_data.channels > 1 {
                audio_data
                    .samples
                    .chunks(audio_data.channels as usize)
                    .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                    .collect()
            } else {
                audio_data.samples
            };

            accumulated.extend_from_slice(&mono);

            if accumulated.len() >= samples_per_update {
                let rms = compute_rms(&accumulated);
                let level_db = if rms > 0.0 { 20.0 * rms.log10() } else { -96.0 };

                broadcast_event(Response::Event {
                    event: EventType::AudioLevelUpdate {
                        device_id: device_id.to_string(),
                        level_db,
                    },
                });

                accumulated.clear();
            }
        } else {
            // No data available, sleep briefly to avoid busy-spinning
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    // Stop capture and clean up
    let _ = backend.stop_capture();
    Ok(())
}

/// Compute RMS amplitude of audio samples.
fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}
