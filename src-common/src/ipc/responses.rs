//! IPC response types.

use serde::{Deserialize, Serialize};

use crate::types::{
    AudioDevice, ConfigValues, CudaStatus, HistoryEntry, ModelStatus, PttStatus, TranscribeStatus,
    TranscriptionResult, VisualizationData,
};

/// IPC response from service to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    // === Success Responses ===
    /// List of audio devices
    Devices { devices: Vec<AudioDevice> },

    /// Current transcription status
    Status(TranscribeStatus),

    /// Whisper model status
    ModelStatus(ModelStatus),

    /// CUDA/GPU status
    CudaStatus(CudaStatus),

    /// Push-to-talk status
    PttStatus(PttStatus),

    /// Persisted configuration values
    ConfigValues(ConfigValues),

    /// Transcription history entries
    History { entries: Vec<HistoryEntry> },

    /// Subscribed to events
    Subscribed,

    /// Generic success
    Ok,

    /// Pong response to ping
    Pong,

    // === Error Response ===
    /// Error occurred
    Error { message: String },

    // === Event Responses (after Subscribe) ===
    /// Real-time event
    Event { event: EventType },
}

/// Event types streamed to subscribed clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventType {
    /// Visualization data update
    VisualizationData(VisualizationData),

    /// Transcription result for a segment
    TranscriptionComplete(TranscriptionResult),

    /// Speech started (segment recording began)
    SpeechStarted,

    /// Speech ended (segment recording stopped)
    SpeechEnded { duration_ms: u64 },

    /// Audio capture state changed
    CaptureStateChanged {
        /// Whether capture is now active
        capturing: bool,
        /// Error message if capture failed
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Model download progress
    ModelDownloadProgress { percent: u8 },

    /// Model download complete
    ModelDownloadComplete { success: bool },

    /// Audio level update from device test capture
    AudioLevelUpdate {
        /// The device being tested
        device_id: String,
        /// Current RMS audio level in dB (negative values, where 0 dB is full scale)
        level_db: f32,
    },

    /// Push-to-talk key pressed
    PttPressed,

    /// Push-to-talk key released
    PttReleased,

    /// Transcription mode changed (Auto vs PTT)
    TranscriptionModeChanged {
        /// The new transcription mode
        mode: crate::types::TranscriptionMode,
    },

    /// Auto mode toggled via hotkey
    AutoModeToggled {
        /// The new transcription mode
        mode: crate::types::TranscriptionMode,
    },

    /// A history entry was deleted
    HistoryEntryDeleted {
        /// The ID of the deleted entry
        id: String,
    },

    /// Service is shutting down
    Shutdown,
}

impl Response {
    /// Create an error response.
    pub fn error(message: impl Into<String>) -> Self {
        Response::Error {
            message: message.into(),
        }
    }

    /// Create a success response.
    pub fn ok() -> Self {
        Response::Ok
    }

    /// Check if this response indicates an error.
    pub fn is_error(&self) -> bool {
        matches!(self, Response::Error { .. })
    }
}
