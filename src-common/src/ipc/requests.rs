//! IPC request types.

use serde::{Deserialize, Serialize};

use crate::types::{AudioSourceType, HotkeyCombination, RecordingMode, TranscriptionMode};

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

    // === Audio Source Configuration ===
    /// Configure audio sources - capture starts automatically when valid sources are set
    SetSources {
        /// Primary audio source ID (mic)
        #[serde(skip_serializing_if = "Option::is_none")]
        source1_id: Option<String>,
        /// Secondary audio source ID (system audio for mixing/AEC)
        #[serde(skip_serializing_if = "Option::is_none")]
        source2_id: Option<String>,
    },

    // === Audio Settings ===
    /// Set acoustic echo cancellation enabled
    SetAecEnabled { enabled: bool },
    /// Set recording mode (mixed or echo-cancel)
    SetRecordingMode { mode: RecordingMode },

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

    // === Configuration ===
    /// Get all persisted configuration values
    GetConfig,

    // === Transcription Mode Control ===
    /// Set the transcription mode (Automatic or PushToTalk)
    SetTranscriptionMode {
        /// The transcription mode to set
        mode: TranscriptionMode,
    },
    /// Set the push-to-talk hotkey combinations
    SetPushToTalkHotkeys {
        /// The hotkey combinations to use for PTT
        hotkeys: Vec<HotkeyCombination>,
    },
    /// Get the current PTT status
    GetPttStatus,
    /// Set the auto-mode toggle hotkeys
    SetAutoToggleHotkeys {
        /// The hotkey combinations to use for toggling auto mode
        hotkeys: Vec<HotkeyCombination>,
    },
    /// Get the current auto-mode toggle hotkeys
    GetAutoToggleHotkeys,
    /// Toggle between Automatic and PushToTalk modes
    ToggleAutoMode,

    // === Clipboard / Auto-Paste ===
    /// Enable or disable automatic paste after transcription
    SetAutoPaste {
        /// Whether auto-paste should be enabled
        enabled: bool,
    },

    // === History Management ===
    /// Get all transcription history entries
    GetHistory,
    /// Delete a single history entry by ID
    DeleteHistoryEntry {
        /// The ID of the history entry to delete
        id: String,
    },

    // === Audio Device Testing ===
    /// Start a lightweight test capture on a device to report audio levels
    TestAudioDevice {
        /// The device ID to test
        device_id: String,
    },
    /// Stop any active audio device test capture
    StopTestAudioDevice,

    // === Platform Permissions ===
    /// Notify the service that the GUI process has confirmed Accessibility permission is granted.
    /// On macOS, the service binary may not have its own Accessibility trust entry (it is an
    /// unsigned helper), so the GUI confirms permission on its behalf. The service uses this
    /// signal to skip its own AXIsProcessTrusted() check and proceed directly to CGEventTapCreate.
    SetAccessibilityPermissionGranted {
        /// Whether the GUI process has confirmed Accessibility access is granted.
        granted: bool,
    },

    // === Service Control ===
    /// Ping for health check
    Ping,
    /// Request service shutdown
    Shutdown,
    /// Get the current runtime mode (development or production)
    GetRuntimeMode,
    /// Register this client as the service owner (only succeeds in production mode)
    RegisterOwner,
}

impl Request {
    /// Validate all parameters in this request.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Request::SetSources {
                source1_id,
                source2_id,
            } => {
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
            Request::TestAudioDevice { device_id } => {
                if device_id.is_empty() {
                    return Err("device_id cannot be empty".to_string());
                }
                Ok(())
            }
            // Other requests have no parameters to validate
            _ => Ok(()),
        }
    }
}
