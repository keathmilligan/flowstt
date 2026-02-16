//! System tray support for FlowSTT.
//!
//! Platform-specific implementations:
//! - Windows: windows.rs
//! - macOS: macos.rs

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

/// Menu item identifiers.
#[allow(dead_code)]
pub mod menu_ids {
    pub const SHOW: &str = "show";
    pub const SETTINGS: &str = "settings";
    pub const ABOUT: &str = "about";
    pub const EXIT: &str = "exit";
}

/// Menu item labels.
#[allow(dead_code)]
pub mod menu_labels {
    pub const SHOW: &str = "Show";
    pub const SETTINGS: &str = "Settings";
    pub const ABOUT: &str = "About";
    pub const EXIT: &str = "Exit";
}

/// Platform-specific tray setup.
#[cfg(windows)]
pub use windows::setup_tray;

#[cfg(target_os = "macos")]
pub use macos::setup_tray;

/// Linux tray - no-op for now.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn setup_tray(_app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
