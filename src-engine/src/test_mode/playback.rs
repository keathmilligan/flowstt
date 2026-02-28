//! WAV audio playback for test mode.
//!
//! Plays WAV files to the system's default audio output device using rodio.
//! The WASAPI loopback capture will pick up this audio for transcription.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::mpsc;

use tracing::debug;

/// Play a WAV file to the default audio output device.
///
/// Returns a `mpsc::Receiver<()>` that signals when playback has finished.
/// The playback runs on a dedicated thread so the caller is not blocked.
///
/// # Errors
///
/// Returns an error if the file cannot be opened, decoded, or if the audio
/// output device is unavailable.
pub fn play_wav(path: &Path) -> Result<mpsc::Receiver<()>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open WAV file: {}", e))?;
    let reader = BufReader::new(file);

    // Open the default audio output device
    let device_sink = rodio::DeviceSinkBuilder::from_default_device()
        .map_err(|e| format!("Failed to find default audio output device: {}", e))?
        .open_sink_or_fallback()
        .map_err(|e| format!("Failed to open audio output device: {}", e))?;

    // rodio::play() accepts Read + Seek (it handles decoding internally)
    let player = rodio::play(device_sink.mixer(), reader)
        .map_err(|e| format!("Failed to start playback: {}", e))?;

    let (done_tx, done_rx) = mpsc::channel();

    // Spawn a thread that owns the device sink and player, waits for playback to finish
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    std::thread::spawn(move || {
        debug!("[TestMode Playback] Playing: {}", file_name);
        player.sleep_until_end();
        debug!("[TestMode Playback] Finished: {}", file_name);
        // device_sink is kept alive by this closure; dropping it stops the output device
        drop(device_sink);
        let _ = done_tx.send(());
    });

    Ok(done_rx)
}
