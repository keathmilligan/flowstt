//! Linux hotkey backend stub.
//!
//! This is a placeholder implementation. Full Linux support using
//! X11/XCB or libinput will be implemented in a future release.

use super::backend::{AutoModeState, HotkeyBackend, HotkeyEvent};
use flowstt_common::HotkeyCombination;
use std::sync::Arc;

/// Linux hotkey backend (stub implementation)
pub struct LinuxHotkeyBackend {
    auto_mode_state: Arc<AutoModeState>,
}

impl LinuxHotkeyBackend {
    pub fn new() -> Self {
        Self {
            auto_mode_state: AutoModeState::shared(),
        }
    }
}

impl HotkeyBackend for LinuxHotkeyBackend {
    fn start(
        &mut self,
        _ptt_hotkeys: Vec<HotkeyCombination>,
        _toggle_hotkeys: Vec<HotkeyCombination>,
    ) -> Result<(), String> {
        Err("Push-to-talk is not yet available on Linux. This feature will be implemented in a future release.".to_string())
    }

    fn stop(&mut self) {
        // No-op for stub
    }

    fn try_recv(&self) -> Option<HotkeyEvent> {
        None
    }

    fn is_running(&self) -> bool {
        false
    }

    fn is_available(&self) -> bool {
        false
    }

    fn unavailable_reason(&self) -> Option<String> {
        Some("Push-to-talk is not yet available on Linux".to_string())
    }

    fn set_auto_mode_active(&mut self, _active: bool) {
        // No-op for stub
    }
}
