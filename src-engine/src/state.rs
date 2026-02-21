//! Global service state management.
//!
//! This module manages the shared state for the FlowSTT service,
//! including transcription status and audio backend state.

use flowstt_common::{
    HotkeyCombination, RecordingMode, RuntimeMode, TranscribeStatus, TranscriptionMode,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Global service state
#[derive(Default)]
pub struct ServiceState {
    /// Current transcription status (capturing, in_speech, queue_depth, error)
    pub transcribe_status: TranscribeStatus,
    /// Whether AEC is enabled
    pub aec_enabled: bool,
    /// Current recording mode
    pub recording_mode: RecordingMode,
    /// Primary audio source ID
    pub source1_id: Option<String>,
    /// Secondary audio source ID (optional)
    pub source2_id: Option<String>,
    /// Current transcription mode (Automatic or PushToTalk)
    pub transcription_mode: TranscriptionMode,
    /// Configured push-to-talk hotkey combinations
    pub ptt_hotkeys: Vec<HotkeyCombination>,
    /// Configured auto-mode toggle hotkeys
    pub auto_toggle_hotkeys: Vec<HotkeyCombination>,
    /// Whether PTT key is currently pressed
    pub is_ptt_active: bool,
    /// Whether auto mode is currently active (for PTT suppression)
    pub auto_mode_active: bool,
    /// Current runtime mode (development or production)
    pub runtime_mode: RuntimeMode,
}

impl ServiceState {
    /// Check if primary audio source is configured
    pub fn has_primary_source(&self) -> bool {
        self.source1_id.is_some()
    }

    /// Check if capture should be active (primary source configured)
    pub fn should_capture(&self) -> bool {
        self.has_primary_source()
    }
}

/// Thread-safe wrapper for service state
pub type SharedState = Arc<Mutex<ServiceState>>;

/// Get the global service state singleton
static SERVICE_STATE: std::sync::OnceLock<SharedState> = std::sync::OnceLock::new();

pub fn get_service_state() -> SharedState {
    SERVICE_STATE
        .get_or_init(|| Arc::new(Mutex::new(ServiceState::default())))
        .clone()
}
