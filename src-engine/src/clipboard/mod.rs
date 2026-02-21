//! Clipboard write, foreground detection, and paste simulation.
//!
//! After each transcription segment completes, this module copies the text to
//! the system clipboard and optionally simulates a paste keystroke into the
//! active foreground application. Paste simulation is suppressed when a FlowSTT
//! window is in the foreground.
//!
//! Platform-specific implementations live in submodules following the same
//! backend-trait pattern used by `crate::hotkey`.

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

use std::time::Duration;
use tracing::{debug, info, warn};

/// Platform-agnostic clipboard and paste backend.
pub trait ClipboardPaster: Send + Sync {
    /// Write plain text to the system clipboard.
    fn write_clipboard(&self, text: &str) -> Result<(), String>;

    /// Check whether the current foreground window belongs to FlowSTT.
    fn is_flowstt_foreground(&self) -> bool;

    /// Simulate a paste keystroke (Ctrl+V / Cmd+V) into the foreground window.
    fn simulate_paste(&self) -> Result<(), String>;
}

/// Create the platform-specific backend.
fn create_backend() -> Box<dyn ClipboardPaster> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsClipboardPaster)
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSClipboardPaster)
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxClipboardPaster)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        compile_error!("Unsupported platform for clipboard/paste");
    }
}

/// Perform the full clipboard-copy-and-paste flow for a transcription result.
///
/// 1. Skip if the text is empty or a "no speech" placeholder.
/// 2. Write the text to the clipboard.
/// 3. If `auto_paste` is enabled and the foreground window is not FlowSTT,
///    wait `delay` and simulate a paste keystroke.
pub fn copy_and_paste(text: &str, auto_paste_enabled: bool, delay_ms: u32) {
    // Skip empty / no-speech results
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "(No speech detected)" {
        return;
    }

    let backend = create_backend();

    // Always write to clipboard
    if let Err(e) = backend.write_clipboard(trimmed) {
        warn!("[Clipboard] Failed to write clipboard: {}", e);
        return;
    }
    debug!("[Clipboard] Text copied to clipboard");

    // Paste only when enabled
    if !auto_paste_enabled {
        return;
    }

    // Suppress paste when FlowSTT is the foreground window
    if backend.is_flowstt_foreground() {
        info!("[Clipboard] FlowSTT is foreground, skipping paste");
        return;
    }

    // Configurable delay before simulating paste
    if delay_ms > 0 {
        std::thread::sleep(Duration::from_millis(delay_ms as u64));
    }

    if let Err(e) = backend.simulate_paste() {
        warn!("[Clipboard] Failed to simulate paste: {}", e);
    } else {
        debug!("[Clipboard] Paste simulated into foreground application");
    }
}
