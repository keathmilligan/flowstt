//! System tray support for FlowSTT.
//!
//! Platform-specific implementations:
//! - Windows: windows.rs

#[cfg(windows)]
pub mod windows;

/// Menu item identifiers.
pub mod menu_ids {
    pub const SHOW: &str = "show";
    pub const SETTINGS: &str = "settings";
    pub const ABOUT: &str = "about";
    pub const EXIT: &str = "exit";
}

/// Menu item labels.
pub mod menu_labels {
    pub const SHOW: &str = "Show";
    pub const SETTINGS: &str = "Settings";
    pub const ABOUT: &str = "About";
    pub const EXIT: &str = "Exit";
}

/// Platform-specific tray setup.
#[cfg(windows)]
pub use windows::setup_tray;

/// Non-Windows platforms - no-op for now.
#[cfg(not(windows))]
pub fn setup_tray(_app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
