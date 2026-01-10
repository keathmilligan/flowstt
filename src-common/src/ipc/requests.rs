//! IPC request types.

use serde::{Deserialize, Serialize};

use crate::types::{AudioSourceType, RecordingMode};

/// Default value for transcription_enabled (true for backwards compatibility)
fn default_transcription_enabled() -> bool {
    true
}

/// IPC request from client to service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    // === Device Enumeration ===
    /// List all audio devices
    ListDevices {
        /// Optional filter by source type
        #[serde(skip_serializing_if = "Option::is_none")]
        source_type: Option<AudioSourceType>,
    },

    // === Transcription Control ===
    /// Start transcription mode
    StartTranscribe {
        /// Primary audio source ID
        #[serde(skip_serializing_if = "Option::is_none")]
        source1_id: Option<String>,
        /// Secondary audio source ID (for mixing/AEC)
        #[serde(skip_serializing_if = "Option::is_none")]
        source2_id: Option<String>,
        /// Enable acoustic echo cancellation
        #[serde(default)]
        aec_enabled: bool,
        /// Recording mode (mixed or echo-cancel)
        #[serde(default)]
        mode: RecordingMode,
        /// Enable transcription (when false, only monitoring/visualization is active)
        #[serde(default = "default_transcription_enabled")]
        transcription_enabled: bool,
    },
    /// Stop transcription mode
    StopTranscribe,

    // === State Queries ===
    /// Get current transcription status
    GetStatus,
    /// Subscribe to real-time events (visualization, transcription results)
    SubscribeEvents,

    // === Model Management ===
    /// Get Whisper model status
    GetModelStatus,
    /// Download the Whisper model
    DownloadModel,
    /// Get CUDA/GPU acceleration status
    GetCudaStatus,

    // === Service Control ===
    /// Ping for health check
    Ping,
    /// Request service shutdown
    Shutdown,
}

impl Request {
    /// Validate all parameters in this request.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Request::StartTranscribe {
                source1_id,
                source2_id,
                ..
            } => {
                // At least one source must be specified
                if source1_id.is_none() && source2_id.is_none() {
                    return Err("At least one audio source must be specified".to_string());
                }
                // Validate source ID format (basic check)
                if let Some(id) = source1_id {
                    if id.is_empty() {
                        return Err("source1_id cannot be empty".to_string());
                    }
                }
                if let Some(id) = source2_id {
                    if id.is_empty() {
                        return Err("source2_id cannot be empty".to_string());
                    }
                }
                Ok(())
            }
            // Other requests have no parameters to validate
            _ => Ok(()),
        }
    }
}
