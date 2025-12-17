//! macOS audio backend module
//!
//! Currently provides a stub implementation. Full CoreAudio support
//! will be added in a future update.

mod stub;

pub use stub::create_backend;
