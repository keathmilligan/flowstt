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

/// Transcription mode - determines how speech segment boundaries are identified.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionMode {
    /// VAD-triggered - speech detection determines segment boundaries
    Automatic,
    /// Manual key-controlled - hotkey press/release determines segment boundaries
    #[default]
    PushToTalk,
}

/// Platform-independent key codes for push-to-talk hotkey configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCode {
    /// Right Alt/Option key (default on macOS)
    RightAlt,
    /// Left Alt/Option key
    LeftAlt,
    /// Right Control key
    RightControl,
    /// Left Control key
    LeftControl,
    /// Right Shift key
    RightShift,
    /// Left Shift key
    LeftShift,
    /// Caps Lock key
    CapsLock,
    /// Function keys F13-F24 (less commonly used)
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
}

impl Default for KeyCode {
    fn default() -> Self {
        KeyCode::RightAlt
    }
}

impl KeyCode {
    /// Get a human-readable display name for the key.
    pub fn display_name(&self) -> &'static str {
        match self {
            KeyCode::RightAlt => "Right Option",
            KeyCode::LeftAlt => "Left Option",
            KeyCode::RightControl => "Right Control",
            KeyCode::LeftControl => "Left Control",
            KeyCode::RightShift => "Right Shift",
            KeyCode::LeftShift => "Left Shift",
            KeyCode::CapsLock => "Caps Lock",
            KeyCode::F13 => "F13",
            KeyCode::F14 => "F14",
            KeyCode::F15 => "F15",
            KeyCode::F16 => "F16",
            KeyCode::F17 => "F17",
            KeyCode::F18 => "F18",
            KeyCode::F19 => "F19",
            KeyCode::F20 => "F20",
        }
    }
}

/// Push-to-talk status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PttStatus {
    /// Current transcription mode
    pub mode: TranscriptionMode,
    /// Configured PTT hotkey
    pub key: KeyCode,
    /// Whether PTT key is currently pressed
    pub is_active: bool,
    /// Whether PTT is available on this platform
    pub available: bool,
    /// Error message if PTT is unavailable (e.g., missing permissions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for PttStatus {
    fn default() -> Self {
        Self {
            mode: TranscriptionMode::default(),
            key: KeyCode::default(),
            is_active: false,
            available: false,
            error: None,
        }
    }
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
    /// Whether audio capture is running (sources configured and valid)
    pub capturing: bool,
    /// Whether currently capturing speech
    pub in_speech: bool,
    /// Number of segments waiting to be transcribed
    pub queue_depth: usize,
    /// Error message if capture failed (e.g., invalid source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Currently configured primary audio source ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source1_id: Option<String>,
    /// Currently configured secondary audio source ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source2_id: Option<String>,
    /// Current transcription mode
    pub transcription_mode: TranscriptionMode,
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
