//! Global hotkey capture for push-to-talk functionality.
//!
//! This module provides platform-specific global hotkey capture:
//! - macOS: CGEventTap API (requires Accessibility permission)
//! - Windows: Raw Input API
//! - Linux: Stub (not yet implemented)

mod backend;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

pub use backend::{HotkeyBackend, HotkeyEvent};

use flowstt_common::HotkeyCombination;
use std::sync::{Arc, Mutex, OnceLock};

/// Global hotkey backend singleton.
static HOTKEY_BACKEND: OnceLock<Arc<Mutex<Box<dyn HotkeyBackend>>>> = OnceLock::new();

/// Initialize the platform-specific hotkey backend.
pub fn init_hotkey_backend() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let backend = macos::MacOSHotkeyBackend::new();
        HOTKEY_BACKEND
            .set(Arc::new(Mutex::new(Box::new(backend))))
            .map_err(|_| "Hotkey backend already initialized".to_string())?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let backend = windows::WindowsHotkeyBackend::new();
        HOTKEY_BACKEND
            .set(Arc::new(Mutex::new(Box::new(backend))))
            .map_err(|_| "Hotkey backend already initialized".to_string())?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let backend = linux::LinuxHotkeyBackend::new();
        HOTKEY_BACKEND
            .set(Arc::new(Mutex::new(Box::new(backend))))
            .map_err(|_| "Hotkey backend already initialized".to_string())?;
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("Unsupported platform for hotkey capture".to_string())
    }
}

/// Get the hotkey backend, initializing if necessary.
pub fn get_hotkey_backend() -> Option<Arc<Mutex<Box<dyn HotkeyBackend>>>> {
    // Try to initialize if not already done
    if HOTKEY_BACKEND.get().is_none() {
        let _ = init_hotkey_backend();
    }
    HOTKEY_BACKEND.get().cloned()
}

/// Start hotkey monitoring with the specified PTT combinations and toggle hotkeys.
pub fn start_hotkey(
    ptt_hotkeys: Vec<HotkeyCombination>,
    toggle_hotkeys: Vec<HotkeyCombination>,
) -> Result<(), String> {
    let backend = get_hotkey_backend().ok_or("Hotkey backend not available")?;
    let mut backend = backend.lock().map_err(|e| format!("Lock error: {}", e))?;
    backend.start(ptt_hotkeys, toggle_hotkeys)
}

/// Stop hotkey monitoring.
pub fn stop_hotkey() {
    if let Some(backend) = get_hotkey_backend() {
        if let Ok(mut backend) = backend.lock() {
            backend.stop();
        }
    }
}

/// Try to receive a hotkey event (non-blocking).
pub fn try_recv_hotkey() -> Option<HotkeyEvent> {
    let backend = get_hotkey_backend()?;
    let backend = backend.lock().ok()?;
    backend.try_recv()
}

/// Check if hotkey capture is available on this platform.
pub fn is_hotkey_available() -> bool {
    if let Some(backend) = get_hotkey_backend() {
        if let Ok(backend) = backend.lock() {
            return backend.is_available();
        }
    }
    false
}

/// Get the reason hotkey capture is unavailable, if any.
pub fn hotkey_unavailable_reason() -> Option<String> {
    let backend = get_hotkey_backend()?;
    let backend = backend.lock().ok()?;
    backend.unavailable_reason()
}

/// Check if macOS Accessibility permission is available.
///
/// On macOS, calls `AXIsProcessTrusted()` in the service's own process context.
/// Returns true on non-macOS platforms (permission not applicable).
pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::check_accessibility_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request macOS Accessibility permission for the service process.
///
/// On macOS, calls `AXIsProcessTrustedWithOptions` with the prompt flag,
/// which causes macOS to show a system dialog asking the user to grant access.
/// Returns the current trust state.
/// Returns true on non-macOS platforms (permission not applicable).
pub fn request_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_accessibility_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Set whether auto mode is active (affects PTT event suppression).
/// When auto mode is active, PTT events are suppressed but toggle events are not.
pub fn set_auto_mode_active(active: bool) {
    if let Some(backend) = get_hotkey_backend() {
        if let Ok(mut backend) = backend.lock() {
            backend.set_auto_mode_active(active);
        }
    }
}
