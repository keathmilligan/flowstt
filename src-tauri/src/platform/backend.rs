//! Platform-agnostic audio backend trait and types
//!
//! This module defines the interface that all platform-specific audio backends
//! must implement. It provides abstraction over different audio systems like
//! PipeWire (Linux), WASAPI (Windows), and CoreAudio (macOS).

use crate::audio::{AudioSourceType, RecordingMode};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Audio device information (platform-independent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAudioDevice {
    /// Device identifier (string to accommodate different platform ID formats)
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Source type (Input or System)
    pub source_type: AudioSourceType,
}

/// Audio samples from capture (platform-independent)
pub struct AudioSamples {
    /// Interleaved audio samples in f32 format
    pub samples: Vec<f32>,
    /// Number of channels (typically 2 for stereo)
    pub channels: u16,
}

/// Platform-agnostic audio backend trait
///
/// All platform-specific audio implementations must implement this trait.
/// The trait is object-safe to allow use with `Box<dyn AudioBackend>`.
pub trait AudioBackend: Send + Sync {
    /// List available input devices (microphones)
    fn list_input_devices(&self) -> Vec<PlatformAudioDevice>;

    /// List available system audio devices (monitors/loopback)
    fn list_system_devices(&self) -> Vec<PlatformAudioDevice>;

    /// Get current sample rate
    fn sample_rate(&self) -> u32;

    /// Start audio capture from specified sources
    ///
    /// # Arguments
    /// * `source1_id` - Optional ID of the first source (typically microphone)
    /// * `source2_id` - Optional ID of the second source (typically system audio)
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` with error message on failure
    fn start_capture_sources(
        &self,
        source1_id: Option<String>,
        source2_id: Option<String>,
    ) -> Result<(), String>;

    /// Stop audio capture
    fn stop_capture(&self) -> Result<(), String>;

    /// Try to receive audio samples (non-blocking)
    ///
    /// Returns `None` if no samples are available, otherwise returns
    /// the next batch of audio samples.
    fn try_recv(&self) -> Option<AudioSamples>;
}

/// Factory function type for creating audio backends
pub type BackendFactory = fn(
    aec_enabled: Arc<Mutex<bool>>,
    recording_mode: Arc<Mutex<RecordingMode>>,
) -> Result<Box<dyn AudioBackend>, String>;

/// Create the platform-appropriate audio backend
///
/// This function is implemented differently per platform using conditional compilation.
/// On Linux, it creates a PipeWire backend. On Windows and macOS, it creates stub
/// backends that return "not implemented" errors.
#[cfg(target_os = "linux")]
pub fn create_backend(
    aec_enabled: Arc<Mutex<bool>>,
    recording_mode: Arc<Mutex<RecordingMode>>,
) -> Result<Box<dyn AudioBackend>, String> {
    super::linux::create_backend(aec_enabled, recording_mode)
}

#[cfg(target_os = "windows")]
pub fn create_backend(
    aec_enabled: Arc<Mutex<bool>>,
    recording_mode: Arc<Mutex<RecordingMode>>,
) -> Result<Box<dyn AudioBackend>, String> {
    super::windows::create_backend(aec_enabled, recording_mode)
}

#[cfg(target_os = "macos")]
pub fn create_backend(
    aec_enabled: Arc<Mutex<bool>>,
    recording_mode: Arc<Mutex<RecordingMode>>,
) -> Result<Box<dyn AudioBackend>, String> {
    super::macos::create_backend(aec_enabled, recording_mode)
}

// Fallback for unsupported platforms
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn create_backend(
    _aec_enabled: Arc<Mutex<bool>>,
    _recording_mode: Arc<Mutex<RecordingMode>>,
) -> Result<Box<dyn AudioBackend>, String> {
    Err("Audio backend not implemented for this platform".to_string())
}
