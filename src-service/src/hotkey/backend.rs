//! Platform-agnostic hotkey backend trait.

use flowstt_common::KeyCode;

/// Event emitted when hotkey state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// The hotkey was pressed
    Pressed,
    /// The hotkey was released
    Released,
}

/// Platform-agnostic hotkey backend interface.
///
/// Implementations capture global keyboard events and filter for the configured
/// push-to-talk hotkey. The backend runs on a separate thread and delivers
/// events via a channel.
pub trait HotkeyBackend: Send {
    /// Start monitoring for the specified hotkey.
    ///
    /// Returns an error if:
    /// - The platform doesn't support global hotkeys
    /// - Required permissions are not granted (e.g., Accessibility on macOS)
    /// - The backend is already running
    fn start(&mut self, key: KeyCode) -> Result<(), String>;

    /// Stop monitoring for hotkey events.
    fn stop(&mut self);

    /// Try to receive a hotkey event (non-blocking).
    ///
    /// Returns `Some(event)` if an event is available, `None` otherwise.
    fn try_recv(&self) -> Option<HotkeyEvent>;

    /// Check if the backend is currently running.
    #[allow(dead_code)]
    fn is_running(&self) -> bool;

    /// Check if the platform supports global hotkeys.
    fn is_available(&self) -> bool;

    /// Get a description of why hotkeys are unavailable, if applicable.
    fn unavailable_reason(&self) -> Option<String>;
}
