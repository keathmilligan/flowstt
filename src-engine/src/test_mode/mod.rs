//! Test Mode
//!
//! Provides a special launch mode for automated testing and demos.
//! When activated via `--test-mode`, enables a tray menu option to select
//! a directory of WAV files, then sequences through them: play each file
//! to the system audio output, simulate PTT press/release, wait for
//! transcription, and advance to the next file.

pub mod playback;

use std::sync::atomic::{AtomicBool, Ordering};

/// Global test mode flag -- set once at startup, never changes.
static TEST_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Global test run in-progress guard.
static TEST_RUN_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Activate test mode. Call once at startup when `--test-mode` flag is present.
pub fn set_test_mode(active: bool) {
    TEST_MODE_ACTIVE.store(active, Ordering::SeqCst);
}

/// Check if test mode is active.
pub fn is_test_mode() -> bool {
    TEST_MODE_ACTIVE.load(Ordering::SeqCst)
}

/// Check if a test run is currently in progress.
pub fn is_test_run_active() -> bool {
    TEST_RUN_ACTIVE.load(Ordering::SeqCst)
}

/// Try to start a test run. Returns false if one is already active.
fn try_start_test_run() -> bool {
    TEST_RUN_ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

/// Mark the current test run as finished.
fn finish_test_run() {
    TEST_RUN_ACTIVE.store(false, Ordering::SeqCst);
}

/// Run the test mode orchestrator on a dedicated thread.
/// Returns an error message if a run is already in progress or the directory is invalid.
pub fn start_test_run(dir: std::path::PathBuf) -> Result<(), String> {
    if !is_test_mode() {
        return Err("Test mode is not active".to_string());
    }

    if !try_start_test_run() {
        tracing::warn!("[TestMode] A test run is already in progress");
        return Err("A test run is already in progress".to_string());
    }

    // Enumerate WAV files
    let mut wav_files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .map(|ext| ext.eq_ignore_ascii_case("wav"))
                        .unwrap_or(false)
            })
            .collect(),
        Err(e) => {
            finish_test_run();
            return Err(format!("Failed to read directory: {}", e));
        }
    };

    if wav_files.is_empty() {
        finish_test_run();
        tracing::warn!("[TestMode] No WAV files found in {:?}", dir);
        return Err("No WAV files found in the selected directory".to_string());
    }

    wav_files.sort();
    let file_count = wav_files.len();
    tracing::info!(
        "[TestMode] Starting test run with {} WAV file(s) from {:?}",
        file_count,
        dir
    );

    std::thread::spawn(move || {
        // Catch panics so we always reset the run-active guard
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_test_sequence(&wav_files);
        }));

        if let Err(e) = result {
            tracing::error!("[TestMode] Orchestrator panicked: {:?}", e);
        }

        finish_test_run();
        tracing::info!("[TestMode] Test run thread exited");
    });

    Ok(())
}

/// Execute the test sequence for a list of WAV files.
fn run_test_sequence(wav_files: &[std::path::PathBuf]) {
    use flowstt_common::ipc::{EventType, Response};
    use std::time::{Duration, Instant};

    let total = wav_files.len();
    let mut success_count = 0u32;
    let mut timeout_count = 0u32;
    let mut skip_count = 0u32;

    // Subscribe to engine events for TranscriptionComplete detection.
    // get_event_sender() returns the tokio broadcast Sender; we subscribe to get a Receiver.
    let mut event_rx = crate::ipc::server::get_event_sender().subscribe();

    for (idx, wav_path) in wav_files.iter().enumerate() {
        let file_name = wav_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        tracing::info!(
            "[TestMode] [{}/{}] Processing: {}",
            idx + 1,
            total,
            file_name
        );

        // 1. Start playback
        let playback_done = match playback::play_wav(wav_path) {
            Ok(done_rx) => done_rx,
            Err(e) => {
                tracing::error!("[TestMode] [{}/{}] Playback error: {}", idx + 1, total, e);
                skip_count += 1;
                continue;
            }
        };

        // 2. Wait 100ms settling delay for loopback capture to start receiving audio
        std::thread::sleep(Duration::from_millis(100));

        // 3. Drain stale events before PTT press so we have a clean slate
        while event_rx.try_recv().is_ok() {}

        // 4. Simulate PTT press (starts capture)
        tracing::debug!("[TestMode] [{}/{}] PTT press", idx + 1, total);
        crate::ptt_controller::handle_ptt_pressed();

        // 5. Wait for playback to complete
        let _ = playback_done.recv();
        tracing::debug!("[TestMode] [{}/{}] Playback finished", idx + 1, total);

        // 6. Wait for any remaining loopback audio to be captured.
        //    WASAPI loopback has inherent latency -- audio in the output buffer
        //    may not have been delivered to the capture side yet.
        std::thread::sleep(Duration::from_millis(500));

        // 7. Simulate PTT release (triggers segment submission for transcription)
        tracing::debug!("[TestMode] [{}/{}] PTT release", idx + 1, total);
        crate::ptt_controller::handle_ptt_released();

        // 8. Wait for TranscriptionComplete or SpeechEnded without a subsequent
        //    TranscriptionComplete (which means Whisper returned empty/no-speech).
        //    Timeout after 30 seconds as a safety net.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut got_transcription = false;
        let mut got_speech_ended = false;

        while Instant::now() < deadline {
            match event_rx.try_recv() {
                Ok(Response::Event {
                    event: EventType::TranscriptionComplete(ref result),
                }) => {
                    tracing::info!(
                        "[TestMode] [{}/{}] Transcription: {}",
                        idx + 1,
                        total,
                        result.text
                    );
                    got_transcription = true;
                    break;
                }
                Ok(Response::Event {
                    event: EventType::SpeechEnded { .. },
                }) => {
                    // Speech segment was submitted. TranscriptionComplete should
                    // follow if Whisper produces a result. Give it extra time
                    // but start a shorter secondary timeout.
                    got_speech_ended = true;
                    tracing::debug!(
                        "[TestMode] [{}/{}] SpeechEnded received, waiting for transcription",
                        idx + 1,
                        total
                    );
                }
                Ok(_) => {
                    // Other event, keep waiting
                    continue;
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    // If we already got SpeechEnded, use a shorter timeout
                    // for the transcription result (10 seconds should be plenty
                    // for Whisper to process a single segment).
                    if got_speech_ended {
                        let speech_ended_deadline = Instant::now() + Duration::from_secs(10);
                        let transcription_received =
                            wait_for_transcription(&mut event_rx, speech_ended_deadline);
                        if let Some(text) = transcription_received {
                            tracing::info!(
                                "[TestMode] [{}/{}] Transcription: {}",
                                idx + 1,
                                total,
                                text
                            );
                            got_transcription = true;
                        } else {
                            tracing::info!(
                                "[TestMode] [{}/{}] No transcription result (empty/no speech)",
                                idx + 1,
                                total,
                            );
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("[TestMode] Event receiver lagged, missed {} events", n);
                    continue;
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    tracing::error!("[TestMode] Event channel closed");
                    return;
                }
            }
        }

        if got_transcription {
            success_count += 1;
        } else if !got_speech_ended {
            tracing::warn!(
                "[TestMode] [{}/{}] Transcription timeout for: {}",
                idx + 1,
                total,
                file_name
            );
            timeout_count += 1;
        }
        // If got_speech_ended but not got_transcription, it was a no-speech result
        // -- counted as neither success nor timeout (just skipped by Whisper)

        // 9. Pause 2 seconds before next file
        if idx + 1 < total {
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    tracing::info!(
        "[TestMode] Test run complete: {}/{} files, {} successful, {} timed out, {} skipped",
        total,
        total,
        success_count,
        timeout_count,
        skip_count
    );
}

/// Wait for a TranscriptionComplete event until the deadline.
/// Returns the transcription text if received, None on timeout or other events.
fn wait_for_transcription(
    event_rx: &mut tokio::sync::broadcast::Receiver<flowstt_common::ipc::Response>,
    deadline: std::time::Instant,
) -> Option<String> {
    use flowstt_common::ipc::{EventType, Response};
    use std::time::Duration;

    while std::time::Instant::now() < deadline {
        match event_rx.try_recv() {
            Ok(Response::Event {
                event: EventType::TranscriptionComplete(ref result),
            }) => {
                return Some(result.text.clone());
            }
            Ok(_) => continue,
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => return None,
        }
    }
    None
}
