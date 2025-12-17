use hound::{WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::processor::{AudioProcessor, SpeechDetector, VisualizationProcessor};

/// Audio source type for capture
#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceType {
    /// Microphone or other input device
    #[default]
    Input,
    /// System audio (monitor/loopback)
    System,
    /// Mixed input and system audio
    Mixed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source_type: AudioSourceType,
}

/// Raw recorded audio data before processing
pub struct RawRecordedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Maximum recording duration in samples (10 minutes at 48kHz stereo)
const MAX_RECORDING_SAMPLES: usize = 48000 * 60 * 10 * 2;

/// Shared state for audio stream
pub struct AudioStreamState {
    // Recording state
    pub recording_samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub is_recording: bool,
    
    // Monitoring state
    pub is_monitoring: bool,
    
    // Visualization processor (always runs when monitoring)
    pub visualization_processor: Option<VisualizationProcessor>,
    
    // Speech processor (always runs when monitoring)
    pub speech_processor: Option<SpeechDetector>,
    
    // Stream control
    pub stream_active: bool,
    
    // Source type for current capture
    pub source_type: AudioSourceType,
}

/// Thread-safe audio state that can be shared with Tauri
#[derive(Clone)]
pub struct RecordingState {
    state: Arc<Mutex<AudioStreamState>>,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AudioStreamState {
                recording_samples: Vec::new(),
                sample_rate: 0,
                channels: 0,
                is_recording: false,
                is_monitoring: false,
                visualization_processor: None,
                speech_processor: None,
                stream_active: false,
                source_type: AudioSourceType::Input,
            })),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.state.lock().unwrap().is_recording
    }

    pub fn is_monitoring(&self) -> bool {
        self.state.lock().unwrap().is_monitoring
    }

    /// Initialize for PipeWire capture with given sample rate and channels
    pub fn init_for_capture(&self, sample_rate: u32, channels: u16, source_type: AudioSourceType) {
        let mut state = self.state.lock().unwrap();
        state.sample_rate = sample_rate;
        state.channels = channels;
        state.source_type = source_type;
        state.stream_active = true;
        // Initialize speech processor with current sample rate
        state.speech_processor = Some(SpeechDetector::new(sample_rate));
    }

    /// Mark capture as stopped
    pub fn mark_capture_stopped(&self) {
        let mut state = self.state.lock().unwrap();
        state.stream_active = false;
    }

    /// Process incoming audio samples from PipeWire
    /// This is called from the audio processing thread
    pub fn process_samples(&self, samples: &[f32], channels: usize, app_handle: &AppHandle) {
        process_audio_samples(samples, channels, &self.state, app_handle);
    }

    /// Get internal state for advanced operations
    pub fn get_state(&self) -> Arc<Mutex<AudioStreamState>> {
        Arc::clone(&self.state)
    }
}

/// Process samples for both recording and visualization
fn process_audio_samples(
    samples: &[f32],
    channels: usize,
    state: &Arc<Mutex<AudioStreamState>>,
    app_handle: &AppHandle,
) {
    // Try to lock without blocking - if we can't get the lock, skip this batch
    if let Ok(mut audio_state) = state.try_lock() {
        // Record samples if recording (with max duration limit)
        if audio_state.is_recording {
            let remaining_capacity = MAX_RECORDING_SAMPLES.saturating_sub(audio_state.recording_samples.len());
            if remaining_capacity > 0 {
                let samples_to_add = samples.len().min(remaining_capacity);
                audio_state.recording_samples.extend_from_slice(&samples[..samples_to_add]);
            }
        }

        // Convert to mono if needed (used for visualization and processing)
        let mono_samples: Vec<f32> = if channels > 1 {
            convert_to_mono(samples, channels)
        } else {
            samples.to_vec()
        };

        // Run speech processor when monitoring is active (always on)
        // Do this first so we can capture metrics for visualization
        if audio_state.is_monitoring {
            if let Some(ref mut processor) = audio_state.speech_processor {
                processor.process(&mono_samples, app_handle);
            }
        }

        // Run visualization processor if monitoring (always runs, independent of processing toggle)
        if audio_state.is_monitoring {
            // Get speech metrics from the speech processor
            let speech_metrics = audio_state.speech_processor.as_ref().map(|p| p.get_metrics());
            
            if let Some(ref mut viz_processor) = audio_state.visualization_processor {
                // Set speech metrics on visualization processor before processing
                if let Some(metrics) = speech_metrics {
                    viz_processor.set_speech_metrics(metrics);
                }
                viz_processor.process(&mono_samples, app_handle);
            }
        }
    }
}

/// Convert multi-channel audio to mono by averaging channels
fn convert_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    samples
        .chunks(channels)
        .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Process raw recorded audio into format suitable for transcription
/// This is CPU-intensive and should be called in a separate thread/task
pub fn process_recorded_audio(raw: RawRecordedAudio) -> Result<Vec<f32>, String> {
    // Convert to mono if stereo
    let mono_samples = if raw.channels > 1 {
        convert_to_mono(&raw.samples, raw.channels as usize)
    } else {
        raw.samples
    };

    // Resample to 16kHz for Whisper
    resample_to_16khz(&mono_samples, raw.sample_rate)
}

/// Resample audio to 16kHz using linear interpolation
/// This is a simple resampler suitable for speech-to-text
fn resample_to_16khz(samples: &[f32], source_rate: u32) -> Result<Vec<f32>, String> {
    const TARGET_RATE: u32 = 16000;

    if source_rate == TARGET_RATE {
        return Ok(samples.to_vec());
    }

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let ratio = source_rate as f64 / TARGET_RATE as f64;
    let output_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos.floor() as usize;
        let frac = src_pos - src_idx as f64;

        let sample = if src_idx + 1 < samples.len() {
            // Linear interpolation between samples
            samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
        } else if src_idx < samples.len() {
            samples[src_idx]
        } else {
            0.0
        };

        output.push(sample);
    }

    Ok(output)
}

/// Save raw audio samples to a WAV file
/// Returns the path to the saved file
pub fn save_to_wav(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    output_path: &PathBuf,
) -> Result<(), String> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = WavWriter::create(output_path, spec)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;

    for &sample in samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

/// Generate a timestamped filename for recording
pub fn generate_recording_filename() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = now.as_secs();

    // Convert to date/time components (UTC)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;

    // Simple date calculation (works for 1970-2099)
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_months: [u64; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0;
    for (i, &days) in days_in_months.iter().enumerate() {
        if remaining_days < days {
            month = i + 1;
            break;
        }
        remaining_days -= days;
    }
    let day = remaining_days + 1;

    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    format!(
        "flowstt-{:04}{:02}{:02}-{:02}{:02}{:02}.wav",
        year, month, day, hour, minute, second
    )
}
