//! Stub audio backend for Windows
//!
//! This module provides a placeholder implementation that compiles successfully
//! but returns "not implemented" errors for all operations. This establishes
//! the infrastructure for future Windows audio support using WASAPI.

use crate::audio::RecordingMode;
use crate::platform::{AudioBackend, AudioSamples, PlatformAudioDevice};
use std::sync::{Arc, Mutex};

/// Stub audio backend for Windows
pub struct StubBackend {
    #[allow(dead_code)]
    aec_enabled: Arc<Mutex<bool>>,
    #[allow(dead_code)]
    recording_mode: Arc<Mutex<RecordingMode>>,
}

impl StubBackend {
    /// Create a new stub backend
    pub fn new(
        aec_enabled: Arc<Mutex<bool>>,
        recording_mode: Arc<Mutex<RecordingMode>>,
    ) -> Result<Self, String> {
        Ok(Self {
            aec_enabled,
            recording_mode,
        })
    }
}

impl AudioBackend for StubBackend {
    fn list_input_devices(&self) -> Vec<PlatformAudioDevice> {
        // Return empty list - no devices available on stub
        Vec::new()
    }

    fn list_system_devices(&self) -> Vec<PlatformAudioDevice> {
        // Return empty list - no devices available on stub
        Vec::new()
    }

    fn sample_rate(&self) -> u32 {
        // Return standard rate even though not implemented
        48000
    }

    fn start_capture_sources(
        &self,
        _source1_id: Option<String>,
        _source2_id: Option<String>,
    ) -> Result<(), String> {
        Err("Audio capture is not yet implemented for Windows. Full WASAPI support coming soon.".to_string())
    }

    fn stop_capture(&self) -> Result<(), String> {
        Err("Audio capture is not yet implemented for Windows.".to_string())
    }

    fn try_recv(&self) -> Option<AudioSamples> {
        // No samples available from stub
        None
    }
}

/// Create a Windows audio backend (stub implementation)
pub fn create_backend(
    aec_enabled: Arc<Mutex<bool>>,
    recording_mode: Arc<Mutex<RecordingMode>>,
) -> Result<Box<dyn AudioBackend>, String> {
    let backend = StubBackend::new(aec_enabled, recording_mode)?;
    Ok(Box::new(backend))
}
