//! Platform-agnostic hotkey backend trait.

use flowstt_common::HotkeyCombination;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Event emitted when hotkey state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// PTT hotkey was pressed
    PttPressed,
    /// PTT hotkey was released
    PttReleased,
    /// Toggle hotkey was pressed
    TogglePressed,
}

/// Platform-agnostic hotkey backend interface.
///
/// Implementations capture global keyboard events and filter for the configured
/// push-to-talk hotkey. The backend runs on a separate thread and delivers
/// events via a channel.
pub trait HotkeyBackend: Send {
    /// Start monitoring for the specified hotkey combinations.
    ///
    /// Returns an error if:
    /// - The platform doesn't support global hotkeys
    /// - Required permissions are not granted (e.g., Accessibility on macOS)
    /// - The backend is already running
    fn start(
        &mut self,
        ptt_hotkeys: Vec<HotkeyCombination>,
        toggle_hotkeys: Vec<HotkeyCombination>,
    ) -> Result<(), String>;

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

    /// Set whether auto mode is active (affects PTT event suppression).
    /// When auto mode is active, PTT events are suppressed but toggle events are not.
    fn set_auto_mode_active(&mut self, active: bool);
}

/// Shared state for PTT suppression across threads.
pub struct AutoModeState {
    /// Whether auto mode is currently active
    pub is_active: AtomicBool,
}

impl AutoModeState {
    pub fn new() -> Self {
        Self {
            is_active: AtomicBool::new(false),
        }
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::SeqCst);
    }
}

impl Default for AutoModeState {
    fn default() -> Self {
        Self::new()
    }
}
