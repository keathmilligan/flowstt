//! FlowSTT Common Library
//!
//! Shared types and IPC protocol for communication between the FlowSTT CLI,
//! service, and GUI components.

pub mod config;
pub mod ipc;
pub mod security;
pub mod types;

pub use config::ThemeMode;
pub use types::*;
