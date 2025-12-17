//! Platform abstraction layer for audio capture
//!
//! This module provides a platform-agnostic interface for audio capture operations.
//! It uses conditional compilation to select the appropriate backend for the target platform:
//! - Linux: PipeWire backend (full functionality)
//! - Windows: Stub backend (not implemented)
//! - macOS: Stub backend (not implemented)

mod backend;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

// Re-export public types
pub use backend::{AudioBackend, AudioSamples, PlatformAudioDevice, create_backend};
