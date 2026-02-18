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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Set by the GUI process when it has confirmed that Accessibility permission is granted.
/// On macOS, the service binary is unsigned and AXIsProcessTrusted() returns false in the
/// service's process context even after the user grants access for the parent app. The GUI
/// calls SetAccessibilityPermissionGranted(true) to signal that permission has been confirmed,
/// allowing the service to skip its own permission check and proceed to CGEventTapCreate.
/// On non-macOS platforms this flag has no effect (permission is not required).
static ACCESSIBILITY_PERMISSION_GRANTED: AtomicBool = AtomicBool::new(false);

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

/// Notify the hotkey module that the GUI process has confirmed Accessibility permission.
/// This allows the service to skip its own AXIsProcessTrusted() check (which always returns
/// false for unsigned helper binaries) and proceed directly to CGEventTapCreate.
pub fn set_accessibility_permission_granted(granted: bool) {
    ACCESSIBILITY_PERMISSION_GRANTED.store(granted, Ordering::SeqCst);
}

/// Check if macOS Accessibility permission is available.
///
/// Returns true if either:
/// - The GUI process has confirmed permission via `set_accessibility_permission_granted()`, or
/// - `AXIsProcessTrusted()` returns true in this process's context.
///
/// Returns true on non-macOS platforms (permission not applicable).
pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        ACCESSIBILITY_PERMISSION_GRANTED.load(Ordering::SeqCst)
            || macos::check_accessibility_permission()
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
