//! Shared types for FlowSTT audio capture and transcription.

use serde::{Deserialize, Serialize};

/// Audio source type for capture.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceType {
    /// Microphone or other input device
    #[default]
    Input,
    /// System audio (monitor/loopback)
    System,
    /// Mixed input and system audio
    Mixed,
}

/// Recording mode - determines how multiple audio sources are combined.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    /// Mix both streams together (default behavior)
    #[default]
    Mixed,
    /// Echo cancellation mode - output only echo-cancelled primary source
    EchoCancel,
}

/// Information about an audio device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    /// Unique identifier (PipeWire node ID, WASAPI endpoint ID, etc.)
    pub id: String,
    /// Display name for UI
    pub name: String,
    /// Type of audio source
    #[serde(default)]
    pub source_type: AudioSourceType,
}

/// Status of the transcription system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscribeStatus {
    /// Whether transcription mode is active
    pub active: bool,
    /// Whether currently capturing speech
    pub in_speech: bool,
    /// Number of segments waiting to be transcribed
    pub queue_depth: usize,
}

/// Status of the Whisper model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    /// Whether the model file exists and is available
    pub available: bool,
    /// Path to the model file
    pub path: String,
}

/// CUDA/GPU acceleration status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaStatus {
    /// Whether the binary was built with CUDA support
    pub build_enabled: bool,
    /// Whether CUDA is available at runtime
    pub runtime_available: bool,
    /// System info string from whisper.cpp
    pub system_info: String,
}

/// A single column of spectrogram data ready for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrogramColumn {
    /// RGB triplets for each pixel row (height * 3 bytes)
    pub colors: Vec<u8>,
}

/// Visualization data for real-time audio display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationData {
    /// Waveform amplitude values (downsampled for display)
    pub waveform: Vec<f32>,
    /// Spectrogram column (RGB color values, if ready)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectrogram: Option<SpectrogramColumn>,
    /// Speech detection metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_metrics: Option<SpeechMetrics>,
}

/// Speech detection metrics for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechMetrics {
    /// RMS amplitude in dB
    pub amplitude_db: f32,
    /// Zero-crossing rate (0.0-1.0)
    pub zcr: f32,
    /// Spectral centroid in Hz
    pub centroid_hz: f32,
    /// Whether speech is currently detected
    pub is_speaking: bool,
    /// Whether voiced onset is pending
    pub voiced_onset_pending: bool,
    /// Whether whisper onset is pending
    pub whisper_onset_pending: bool,
    /// Whether a transient was detected
    pub is_transient: bool,
    /// Whether this is lookback-determined speech
    pub is_lookback_speech: bool,
    /// Whether this is a word break
    pub is_word_break: bool,
}

/// Transcription result for a speech segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Path to the saved audio file (if saved)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_path: Option<String>,
}
