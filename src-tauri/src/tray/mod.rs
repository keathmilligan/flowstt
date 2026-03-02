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
    pub const ALWAYS_ON_TOP: &str = "always_on_top";
    pub const SETTINGS: &str = "settings";
    pub const LOGS: &str = "logs";
    pub const ABOUT: &str = "about";
    pub const RUN_TEST: &str = "run_test";
    pub const EXIT: &str = "exit";
}

/// Menu item labels.
#[allow(dead_code)]
pub mod menu_labels {
    pub const SHOW: &str = "Show";
    pub const ALWAYS_ON_TOP: &str = "Always on Top";
    pub const SETTINGS: &str = "Settings";
    pub const LOGS: &str = "Logs";
    pub const ABOUT: &str = "About";
    pub const RUN_TEST: &str = "Run Test (WAV Directory)...";
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

/// Shut down the engine directly (in-process).
/// Used by the tray Exit handler to stop the engine before exiting the app.
fn shutdown_engine() {
    flowstt_engine::request_shutdown();
    flowstt_engine::cleanup();
}
