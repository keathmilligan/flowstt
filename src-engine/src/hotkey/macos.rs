//! macOS hotkey backend using CGEventTap.
//!
//! This implementation uses the Core Graphics Event Tap API to monitor
//! global keyboard events. It requires Accessibility permission to function.

use super::backend::{AutoModeState, HotkeyBackend, HotkeyEvent};
use flowstt_common::{HotkeyCombination, KeyCode};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info};

#[allow(dead_code)]
mod keycode {
    pub const A: u16 = 0x00;
    pub const S: u16 = 0x01;
    pub const D: u16 = 0x02;
    pub const F: u16 = 0x03;
    pub const H: u16 = 0x04;
    pub const G: u16 = 0x05;
    pub const Z: u16 = 0x06;
    pub const X: u16 = 0x07;
    pub const C: u16 = 0x08;
    pub const V: u16 = 0x09;
    pub const B: u16 = 0x0B;
    pub const Q: u16 = 0x0C;
    pub const W: u16 = 0x0D;
    pub const E: u16 = 0x0E;
    pub const R: u16 = 0x0F;
    pub const Y: u16 = 0x10;
    pub const T: u16 = 0x11;
    pub const DIGIT_1: u16 = 0x12;
    pub const DIGIT_2: u16 = 0x13;
    pub const DIGIT_3: u16 = 0x14;
    pub const DIGIT_4: u16 = 0x15;
    pub const DIGIT_6: u16 = 0x16;
    pub const DIGIT_5: u16 = 0x17;
    pub const EQUAL: u16 = 0x18;
    pub const DIGIT_9: u16 = 0x19;
    pub const DIGIT_7: u16 = 0x1A;
    pub const MINUS: u16 = 0x1B;
    pub const DIGIT_8: u16 = 0x1C;
    pub const DIGIT_0: u16 = 0x1D;
    pub const BRACKET_RIGHT: u16 = 0x1E;
    pub const O: u16 = 0x1F;
    pub const U: u16 = 0x20;
    pub const BRACKET_LEFT: u16 = 0x21;
    pub const I: u16 = 0x22;
    pub const P: u16 = 0x23;
    pub const ENTER: u16 = 0x24;
    pub const L: u16 = 0x25;
    pub const J: u16 = 0x26;
    pub const QUOTE: u16 = 0x27;
    pub const K: u16 = 0x28;
    pub const SEMICOLON: u16 = 0x29;
    pub const BACKSLASH: u16 = 0x2A;
    pub const COMMA: u16 = 0x2B;
    pub const SLASH: u16 = 0x2C;
    pub const N: u16 = 0x2D;
    pub const M: u16 = 0x2E;
    pub const PERIOD: u16 = 0x2F;
    pub const TAB: u16 = 0x30;
    pub const SPACE: u16 = 0x31;
    pub const BACKQUOTE: u16 = 0x32;
    pub const BACKSPACE: u16 = 0x33;
    pub const ESCAPE: u16 = 0x35;
    pub const RIGHT_OPTION: u16 = 0x3D;
    pub const LEFT_OPTION: u16 = 0x3A;
    pub const RIGHT_CONTROL: u16 = 0x3E;
    pub const LEFT_CONTROL: u16 = 0x3B;
    pub const RIGHT_SHIFT: u16 = 0x3C;
    pub const LEFT_SHIFT: u16 = 0x38;
    pub const CAPS_LOCK: u16 = 0x39;
    pub const LEFT_META: u16 = 0x37;
    pub const RIGHT_META: u16 = 0x36;
    pub const F1: u16 = 0x7A;
    pub const F2: u16 = 0x78;
    pub const F3: u16 = 0x63;
    pub const F4: u16 = 0x76;
    pub const F5: u16 = 0x60;
    pub const F6: u16 = 0x61;
    pub const F7: u16 = 0x62;
    pub const F8: u16 = 0x64;
    pub const F9: u16 = 0x65;
    pub const F10: u16 = 0x6D;
    pub const F11: u16 = 0x67;
    pub const F12: u16 = 0x6F;
    pub const F13: u16 = 0x69;
    pub const F14: u16 = 0x6B;
    pub const F15: u16 = 0x71;
    pub const F16: u16 = 0x6A;
    pub const F17: u16 = 0x40;
    pub const F18: u16 = 0x4F;
    pub const F19: u16 = 0x50;
    pub const F20: u16 = 0x5A;
    pub const F21: u16 = 0x5C;
    pub const F22: u16 = 0x58;
    pub const F23: u16 = 0x56;
    pub const F24: u16 = 0x57;
    pub const HOME: u16 = 0x73;
    pub const PAGE_UP: u16 = 0x74;
    pub const FORWARD_DELETE: u16 = 0x75;
    pub const END: u16 = 0x77;
    pub const PAGE_DOWN: u16 = 0x79;
    pub const ARROW_LEFT: u16 = 0x7B;
    pub const ARROW_RIGHT: u16 = 0x7C;
    pub const ARROW_DOWN: u16 = 0x7D;
    pub const ARROW_UP: u16 = 0x7E;
    pub const NUM_LOCK: u16 = 0x47;
    pub const NUMPAD_EQUAL: u16 = 0x51;
    pub const NUMPAD_DIVIDE: u16 = 0x4B;
    pub const NUMPAD_MULTIPLY: u16 = 0x43;
    pub const NUMPAD_SUBTRACT: u16 = 0x4E;
    pub const NUMPAD_ADD: u16 = 0x45;
    pub const NUMPAD_ENTER: u16 = 0x4C;
    pub const NUMPAD_DECIMAL: u16 = 0x41;
    pub const NUMPAD_0: u16 = 0x52;
    pub const NUMPAD_1: u16 = 0x53;
    pub const NUMPAD_2: u16 = 0x54;
    pub const NUMPAD_3: u16 = 0x55;
    pub const NUMPAD_4: u16 = 0x56;
    pub const NUMPAD_5: u16 = 0x57;
    pub const NUMPAD_6: u16 = 0x58;
    pub const NUMPAD_7: u16 = 0x59;
    pub const NUMPAD_8: u16 = 0x5B;
    pub const NUMPAD_9: u16 = 0x5C;
    pub const INSERT: u16 = 0x72;
    pub const PRINT_SCREEN: u16 = 0x6B;
    pub const SCROLL_LOCK: u16 = 0x71;
    pub const PAUSE: u16 = 0x71;
}

#[allow(dead_code)]
fn keycode_to_macos(key: KeyCode) -> u16 {
    match key {
        KeyCode::RightAlt => keycode::RIGHT_OPTION,
        KeyCode::LeftAlt => keycode::LEFT_OPTION,
        KeyCode::RightControl => keycode::RIGHT_CONTROL,
        KeyCode::LeftControl => keycode::LEFT_CONTROL,
        KeyCode::RightShift => keycode::RIGHT_SHIFT,
        KeyCode::LeftShift => keycode::LEFT_SHIFT,
        KeyCode::CapsLock => keycode::CAPS_LOCK,
        KeyCode::LeftMeta => keycode::LEFT_META,
        KeyCode::RightMeta => keycode::RIGHT_META,
        KeyCode::F1 => keycode::F1,
        KeyCode::F2 => keycode::F2,
        KeyCode::F3 => keycode::F3,
        KeyCode::F4 => keycode::F4,
        KeyCode::F5 => keycode::F5,
        KeyCode::F6 => keycode::F6,
        KeyCode::F7 => keycode::F7,
        KeyCode::F8 => keycode::F8,
        KeyCode::F9 => keycode::F9,
        KeyCode::F10 => keycode::F10,
        KeyCode::F11 => keycode::F11,
        KeyCode::F12 => keycode::F12,
        KeyCode::F13 => keycode::F13,
        KeyCode::F14 => keycode::F14,
        KeyCode::F15 => keycode::F15,
        KeyCode::F16 => keycode::F16,
        KeyCode::F17 => keycode::F17,
        KeyCode::F18 => keycode::F18,
        KeyCode::F19 => keycode::F19,
        KeyCode::F20 => keycode::F20,
        KeyCode::F21 => keycode::F21,
        KeyCode::F22 => keycode::F22,
        KeyCode::F23 => keycode::F23,
        KeyCode::F24 => keycode::F24,
        KeyCode::KeyA => keycode::A,
        KeyCode::KeyS => keycode::S,
        KeyCode::KeyD => keycode::D,
        KeyCode::KeyF => keycode::F,
        KeyCode::KeyH => keycode::H,
        KeyCode::KeyG => keycode::G,
        KeyCode::KeyZ => keycode::Z,
        KeyCode::KeyX => keycode::X,
        KeyCode::KeyC => keycode::C,
        KeyCode::KeyV => keycode::V,
        KeyCode::KeyB => keycode::B,
        KeyCode::KeyQ => keycode::Q,
        KeyCode::KeyW => keycode::W,
        KeyCode::KeyE => keycode::E,
        KeyCode::KeyR => keycode::R,
        KeyCode::KeyY => keycode::Y,
        KeyCode::KeyT => keycode::T,
        KeyCode::KeyO => keycode::O,
        KeyCode::KeyU => keycode::U,
        KeyCode::KeyI => keycode::I,
        KeyCode::KeyP => keycode::P,
        KeyCode::KeyL => keycode::L,
        KeyCode::KeyJ => keycode::J,
        KeyCode::KeyK => keycode::K,
        KeyCode::KeyN => keycode::N,
        KeyCode::KeyM => keycode::M,
        KeyCode::Digit1 => keycode::DIGIT_1,
        KeyCode::Digit2 => keycode::DIGIT_2,
        KeyCode::Digit3 => keycode::DIGIT_3,
        KeyCode::Digit4 => keycode::DIGIT_4,
        KeyCode::Digit5 => keycode::DIGIT_5,
        KeyCode::Digit6 => keycode::DIGIT_6,
        KeyCode::Digit7 => keycode::DIGIT_7,
        KeyCode::Digit8 => keycode::DIGIT_8,
        KeyCode::Digit9 => keycode::DIGIT_9,
        KeyCode::Digit0 => keycode::DIGIT_0,
        KeyCode::ArrowUp => keycode::ARROW_UP,
        KeyCode::ArrowDown => keycode::ARROW_DOWN,
        KeyCode::ArrowLeft => keycode::ARROW_LEFT,
        KeyCode::ArrowRight => keycode::ARROW_RIGHT,
        KeyCode::Home => keycode::HOME,
        KeyCode::End => keycode::END,
        KeyCode::PageUp => keycode::PAGE_UP,
        KeyCode::PageDown => keycode::PAGE_DOWN,
        KeyCode::Insert => keycode::INSERT,
        KeyCode::Delete => keycode::FORWARD_DELETE,
        KeyCode::Escape => keycode::ESCAPE,
        KeyCode::Tab => keycode::TAB,
        KeyCode::Space => keycode::SPACE,
        KeyCode::Enter => keycode::ENTER,
        KeyCode::Backspace => keycode::BACKSPACE,
        KeyCode::PrintScreen => keycode::PRINT_SCREEN,
        KeyCode::ScrollLock => keycode::SCROLL_LOCK,
        KeyCode::Pause => keycode::PAUSE,
        KeyCode::Minus => keycode::MINUS,
        KeyCode::Equal => keycode::EQUAL,
        KeyCode::BracketLeft => keycode::BRACKET_LEFT,
        KeyCode::BracketRight => keycode::BRACKET_RIGHT,
        KeyCode::Backslash => keycode::BACKSLASH,
        KeyCode::Semicolon => keycode::SEMICOLON,
        KeyCode::Quote => keycode::QUOTE,
        KeyCode::Backquote => keycode::BACKQUOTE,
        KeyCode::Comma => keycode::COMMA,
        KeyCode::Period => keycode::PERIOD,
        KeyCode::Slash => keycode::SLASH,
        KeyCode::Numpad0 => keycode::NUMPAD_0,
        KeyCode::Numpad1 => keycode::NUMPAD_1,
        KeyCode::Numpad2 => keycode::NUMPAD_2,
        KeyCode::Numpad3 => keycode::NUMPAD_3,
        KeyCode::Numpad4 => keycode::NUMPAD_4,
        KeyCode::Numpad5 => keycode::NUMPAD_5,
        KeyCode::Numpad6 => keycode::NUMPAD_6,
        KeyCode::Numpad7 => keycode::NUMPAD_7,
        KeyCode::Numpad8 => keycode::NUMPAD_8,
        KeyCode::Numpad9 => keycode::NUMPAD_9,
        KeyCode::NumpadMultiply => keycode::NUMPAD_MULTIPLY,
        KeyCode::NumpadAdd => keycode::NUMPAD_ADD,
        KeyCode::NumpadSubtract => keycode::NUMPAD_SUBTRACT,
        KeyCode::NumpadDecimal => keycode::NUMPAD_DECIMAL,
        KeyCode::NumpadDivide => keycode::NUMPAD_DIVIDE,
        KeyCode::NumLock => keycode::NUM_LOCK,
    }
}

fn macos_to_keycode(keycode: u16) -> Option<KeyCode> {
    match keycode {
        keycode::RIGHT_OPTION => Some(KeyCode::RightAlt),
        keycode::LEFT_OPTION => Some(KeyCode::LeftAlt),
        keycode::RIGHT_CONTROL => Some(KeyCode::RightControl),
        keycode::LEFT_CONTROL => Some(KeyCode::LeftControl),
        keycode::RIGHT_SHIFT => Some(KeyCode::RightShift),
        keycode::LEFT_SHIFT => Some(KeyCode::LeftShift),
        keycode::CAPS_LOCK => Some(KeyCode::CapsLock),
        keycode::LEFT_META => Some(KeyCode::LeftMeta),
        keycode::RIGHT_META => Some(KeyCode::RightMeta),
        keycode::F1 => Some(KeyCode::F1),
        keycode::F2 => Some(KeyCode::F2),
        keycode::F3 => Some(KeyCode::F3),
        keycode::F4 => Some(KeyCode::F4),
        keycode::F5 => Some(KeyCode::F5),
        keycode::F6 => Some(KeyCode::F6),
        keycode::F7 => Some(KeyCode::F7),
        keycode::F8 => Some(KeyCode::F8),
        keycode::F9 => Some(KeyCode::F9),
        keycode::F10 => Some(KeyCode::F10),
        keycode::F11 => Some(KeyCode::F11),
        keycode::F12 => Some(KeyCode::F12),
        keycode::F13 => Some(KeyCode::F13),
        keycode::F14 => Some(KeyCode::F14),
        keycode::F15 => Some(KeyCode::F15),
        keycode::F16 => Some(KeyCode::F16),
        keycode::F17 => Some(KeyCode::F17),
        keycode::F18 => Some(KeyCode::F18),
        keycode::F19 => Some(KeyCode::F19),
        keycode::F20 => Some(KeyCode::F20),
        keycode::F21 => Some(KeyCode::F21),
        keycode::F22 => Some(KeyCode::F22),
        keycode::F23 => Some(KeyCode::F23),
        keycode::F24 => Some(KeyCode::F24),
        keycode::A => Some(KeyCode::KeyA),
        keycode::S => Some(KeyCode::KeyS),
        keycode::D => Some(KeyCode::KeyD),
        keycode::F => Some(KeyCode::KeyF),
        keycode::H => Some(KeyCode::KeyH),
        keycode::G => Some(KeyCode::KeyG),
        keycode::Z => Some(KeyCode::KeyZ),
        keycode::X => Some(KeyCode::KeyX),
        keycode::C => Some(KeyCode::KeyC),
        keycode::V => Some(KeyCode::KeyV),
        keycode::B => Some(KeyCode::KeyB),
        keycode::Q => Some(KeyCode::KeyQ),
        keycode::W => Some(KeyCode::KeyW),
        keycode::E => Some(KeyCode::KeyE),
        keycode::R => Some(KeyCode::KeyR),
        keycode::Y => Some(KeyCode::KeyY),
        keycode::T => Some(KeyCode::KeyT),
        keycode::O => Some(KeyCode::KeyO),
        keycode::U => Some(KeyCode::KeyU),
        keycode::I => Some(KeyCode::KeyI),
        keycode::P => Some(KeyCode::KeyP),
        keycode::L => Some(KeyCode::KeyL),
        keycode::J => Some(KeyCode::KeyJ),
        keycode::K => Some(KeyCode::KeyK),
        keycode::N => Some(KeyCode::KeyN),
        keycode::M => Some(KeyCode::KeyM),
        keycode::DIGIT_1 => Some(KeyCode::Digit1),
        keycode::DIGIT_2 => Some(KeyCode::Digit2),
        keycode::DIGIT_3 => Some(KeyCode::Digit3),
        keycode::DIGIT_4 => Some(KeyCode::Digit4),
        keycode::DIGIT_5 => Some(KeyCode::Digit5),
        keycode::DIGIT_6 => Some(KeyCode::Digit6),
        keycode::DIGIT_7 => Some(KeyCode::Digit7),
        keycode::DIGIT_8 => Some(KeyCode::Digit8),
        keycode::DIGIT_9 => Some(KeyCode::Digit9),
        keycode::DIGIT_0 => Some(KeyCode::Digit0),
        keycode::ARROW_UP => Some(KeyCode::ArrowUp),
        keycode::ARROW_DOWN => Some(KeyCode::ArrowDown),
        keycode::ARROW_LEFT => Some(KeyCode::ArrowLeft),
        keycode::ARROW_RIGHT => Some(KeyCode::ArrowRight),
        keycode::HOME => Some(KeyCode::Home),
        keycode::END => Some(KeyCode::End),
        keycode::PAGE_UP => Some(KeyCode::PageUp),
        keycode::PAGE_DOWN => Some(KeyCode::PageDown),
        keycode::INSERT => Some(KeyCode::Insert),
        keycode::FORWARD_DELETE => Some(KeyCode::Delete),
        keycode::ESCAPE => Some(KeyCode::Escape),
        keycode::TAB => Some(KeyCode::Tab),
        keycode::SPACE => Some(KeyCode::Space),
        keycode::ENTER => Some(KeyCode::Enter),
        keycode::BACKSPACE => Some(KeyCode::Backspace),
        keycode::MINUS => Some(KeyCode::Minus),
        keycode::EQUAL => Some(KeyCode::Equal),
        keycode::BRACKET_LEFT => Some(KeyCode::BracketLeft),
        keycode::BRACKET_RIGHT => Some(KeyCode::BracketRight),
        keycode::BACKSLASH => Some(KeyCode::Backslash),
        keycode::SEMICOLON => Some(KeyCode::Semicolon),
        keycode::QUOTE => Some(KeyCode::Quote),
        keycode::BACKQUOTE => Some(KeyCode::Backquote),
        keycode::COMMA => Some(KeyCode::Comma),
        keycode::PERIOD => Some(KeyCode::Period),
        keycode::SLASH => Some(KeyCode::Slash),
        keycode::NUMPAD_0 => Some(KeyCode::Numpad0),
        keycode::NUMPAD_1 => Some(KeyCode::Numpad1),
        keycode::NUMPAD_2 => Some(KeyCode::Numpad2),
        keycode::NUMPAD_3 => Some(KeyCode::Numpad3),
        keycode::NUMPAD_MULTIPLY => Some(KeyCode::NumpadMultiply),
        keycode::NUMPAD_ADD => Some(KeyCode::NumpadAdd),
        keycode::NUMPAD_SUBTRACT => Some(KeyCode::NumpadSubtract),
        keycode::NUMPAD_DECIMAL => Some(KeyCode::NumpadDecimal),
        keycode::NUMPAD_DIVIDE => Some(KeyCode::NumpadDivide),
        keycode::NUM_LOCK => Some(KeyCode::NumLock),
        _ => None,
    }
}

/// macOS hotkey backend using CGEventTap
pub struct MacOSHotkeyBackend {
    /// Whether the backend is currently running
    running: Arc<AtomicBool>,
    /// Channel for receiving hotkey events
    receiver: Option<Receiver<HotkeyEvent>>,
    /// Handle to the monitoring thread
    thread_handle: Option<JoinHandle<()>>,
    /// Last known unavailability reason
    unavailable_reason: Option<String>,
    /// Auto mode state for PTT suppression (shared with event tap thread)
    auto_mode_state: Arc<AutoModeState>,
}

/// Returns true if the process currently has macOS Accessibility permission.
/// This is safe to call at any time and does not show a system dialog.
pub fn check_accessibility_permission() -> bool {
    unsafe { macos_ffi::AXIsProcessTrusted() }
}

/// Prompt macOS to show the Accessibility permission dialog for this process.
/// This calls AXIsProcessTrustedWithOptions with kAXTrustedCheckOptionPrompt=true,
/// which causes macOS to add this process (flowstt-app) to the Accessibility list
/// in System Settings. The user must then toggle it on. Returns the current trust state.
pub fn request_accessibility_permission() -> bool {
    unsafe {
        let key = macos_ffi::kAXTrustedCheckOptionPrompt as macos_ffi::CFTypeRef;
        let value = macos_ffi::kCFBooleanTrue;

        let options = macos_ffi::CFDictionaryCreate(
            std::ptr::null(),
            &key,
            &value,
            1,
            &macos_ffi::kCFTypeDictionaryKeyCallBacks,
            &macos_ffi::kCFTypeDictionaryValueCallBacks,
        );

        if options.is_null() {
            error!("[Hotkey] Failed to create CFDictionary for AXIsProcessTrustedWithOptions");
            return macos_ffi::AXIsProcessTrusted();
        }

        let trusted = macos_ffi::AXIsProcessTrustedWithOptions(options);
        macos_ffi::CFRelease(options as macos_ffi::CFTypeRef);
        info!(
            "[Hotkey] AXIsProcessTrustedWithOptions(prompt=true) returned: {}",
            trusted
        );
        trusted
    }
}

impl MacOSHotkeyBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            receiver: None,
            thread_handle: None,
            unavailable_reason: None,
            auto_mode_state: AutoModeState::shared(),
        }
    }
}

impl HotkeyBackend for MacOSHotkeyBackend {
    fn start(
        &mut self,
        ptt_hotkeys: Vec<HotkeyCombination>,
        toggle_hotkeys: Vec<HotkeyCombination>,
    ) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Hotkey backend already running".to_string());
        }

        if ptt_hotkeys.is_empty() && toggle_hotkeys.is_empty() {
            return Err("No hotkey combinations configured".to_string());
        }

        // Check Accessibility permission directly in this process's context.
        // The Tauri app process creates the CGEventTap, so it must be the
        // process granted Accessibility access by the user.
        if !super::check_accessibility_permission() {
            let msg = "Push-to-Talk requires Accessibility permission to detect hotkeys. Grant permission to FlowSTT in System Settings > Privacy & Security > Accessibility, then restart FlowSTT.".to_string();
            info!("[Hotkey] Accessibility permission not granted: {}", msg);
            self.unavailable_reason = Some(msg.clone());
            return Err(msg);
        }

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let auto_mode_state = self.auto_mode_state.clone();

        let handle = thread::spawn(move || {
            info!(
                "[Hotkey] Starting macOS event tap for {} PTT hotkey(s), {} toggle hotkey(s)",
                ptt_hotkeys.len(),
                toggle_hotkeys.len()
            );

            if let Err(e) = run_event_tap(
                running.clone(),
                sender,
                ptt_hotkeys,
                toggle_hotkeys,
                auto_mode_state,
            ) {
                error!("[Hotkey] Event tap error: {}", e);
            }

            info!("[Hotkey] Event tap thread exiting");
        });

        self.thread_handle = Some(handle);
        self.unavailable_reason = None;

        Ok(())
    }

    fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        info!("[Hotkey] Stopping hotkey backend");
        self.running.store(false, Ordering::SeqCst);

        // The thread will exit when it detects running is false
        if let Some(handle) = self.thread_handle.take() {
            // Give the thread a moment to exit gracefully
            let _ = handle.join();
        }

        self.receiver = None;
    }

    fn try_recv(&self) -> Option<HotkeyEvent> {
        self.receiver.as_ref()?.try_recv().ok()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn is_available(&self) -> bool {
        // macOS PTT is always available - permission will be requested when needed
        true
    }

    fn unavailable_reason(&self) -> Option<String> {
        // Only report unavailability if we tried to start and failed
        self.unavailable_reason.clone()
    }

    fn set_auto_mode_active(&mut self, active: bool) {
        self.auto_mode_state.set_active(active);
        debug!("[Hotkey] Auto mode PTT suppression: {}", active);
    }
}

impl Drop for MacOSHotkeyBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run the CGEventTap on this thread
fn run_event_tap(
    running: Arc<AtomicBool>,
    sender: Sender<HotkeyEvent>,
    ptt_hotkeys: Vec<HotkeyCombination>,
    toggle_hotkeys: Vec<HotkeyCombination>,
    auto_mode_state: Arc<AutoModeState>,
) -> Result<(), String> {
    unsafe {
        let event_mask = (1 << macos_ffi::kCGEventKeyDown)
            | (1 << macos_ffi::kCGEventKeyUp)
            | (1 << macos_ffi::kCGEventFlagsChanged);

        let context = Box::new(EventTapContext {
            sender,
            ptt_hotkeys,
            toggle_hotkeys,
            pressed_keys: Mutex::new(HashSet::new()),
            any_ptt_matched: AtomicBool::new(false),
            any_toggle_matched: AtomicBool::new(false),
            auto_mode_state,
        });
        let context_ptr = Box::into_raw(context);

        let tap = macos_ffi::CGEventTapCreate(
            macos_ffi::kCGSessionEventTap,
            macos_ffi::kCGHeadInsertEventTap,
            macos_ffi::kCGEventTapOptionListenOnly,
            event_mask,
            event_tap_callback,
            context_ptr as *mut std::ffi::c_void,
        );

        if tap.is_null() {
            let _ = Box::from_raw(context_ptr);
            return Err("Failed to create event tap. Check Accessibility permissions.".to_string());
        }

        let run_loop_source = macos_ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);

        if run_loop_source.is_null() {
            macos_ffi::CFRelease(tap as *const std::ffi::c_void);
            let _ = Box::from_raw(context_ptr);
            return Err("Failed to create run loop source".to_string());
        }

        let run_loop = macos_ffi::CFRunLoopGetCurrent();
        macos_ffi::CFRunLoopAddSource(run_loop, run_loop_source, macos_ffi::kCFRunLoopCommonModes);

        macos_ffi::CGEventTapEnable(tap, true);

        debug!("[Hotkey] Event tap created and enabled");

        while running.load(Ordering::SeqCst) {
            let result = macos_ffi::CFRunLoopRunInMode(macos_ffi::kCFRunLoopDefaultMode, 0.1, true);

            if result == macos_ffi::kCFRunLoopRunFinished {
                break;
            }
        }

        macos_ffi::CGEventTapEnable(tap, false);
        macos_ffi::CFRunLoopRemoveSource(
            run_loop,
            run_loop_source,
            macos_ffi::kCFRunLoopCommonModes,
        );
        macos_ffi::CFRelease(run_loop_source as *const std::ffi::c_void);
        macos_ffi::CFRelease(tap as *const std::ffi::c_void);
        let _ = Box::from_raw(context_ptr);

        debug!("[Hotkey] Event tap cleaned up");
    }

    Ok(())
}

struct EventTapContext {
    sender: Sender<HotkeyEvent>,
    ptt_hotkeys: Vec<HotkeyCombination>,
    toggle_hotkeys: Vec<HotkeyCombination>,
    pressed_keys: Mutex<HashSet<KeyCode>>,
    any_ptt_matched: AtomicBool,
    any_toggle_matched: AtomicBool,
    auto_mode_state: Arc<AutoModeState>,
}

extern "C" fn event_tap_callback(
    _proxy: macos_ffi::CGEventTapProxy,
    event_type: macos_ffi::CGEventType,
    event: macos_ffi::CGEventRef,
    user_info: *mut std::ffi::c_void,
) -> macos_ffi::CGEventRef {
    let context = unsafe { &*(user_info as *const EventTapContext) };

    let keycode = unsafe {
        macos_ffi::CGEventGetIntegerValueField(event, macos_ffi::kCGKeyboardEventKeycode)
    } as u16;

    let flags = unsafe { macos_ffi::CGEventGetFlags(event) };

    if event_type == macos_ffi::kCGEventFlagsChanged {
        handle_modifier_key(context, keycode, flags);
    } else if event_type == macos_ffi::kCGEventKeyDown || event_type == macos_ffi::kCGEventKeyUp {
        let is_key_down = event_type == macos_ffi::kCGEventKeyDown;
        handle_regular_key(context, keycode, is_key_down);
    }

    check_combinations(context);

    event
}

fn handle_modifier_key(context: &EventTapContext, keycode: u16, flags: macos_ffi::CGEventFlags) {
    let key_code = match macos_to_keycode(keycode) {
        Some(k) => k,
        None => return,
    };

    let is_pressed = match keycode {
        keycode::RIGHT_OPTION | keycode::LEFT_OPTION => {
            (flags & macos_ffi::kCGEventFlagMaskAlternate) != 0
        }
        keycode::RIGHT_CONTROL | keycode::LEFT_CONTROL => {
            (flags & macos_ffi::kCGEventFlagMaskControl) != 0
        }
        keycode::RIGHT_SHIFT | keycode::LEFT_SHIFT => {
            (flags & macos_ffi::kCGEventFlagMaskShift) != 0
        }
        keycode::CAPS_LOCK => (flags & macos_ffi::kCGEventFlagMaskAlphaShift) != 0,
        keycode::LEFT_META | keycode::RIGHT_META => {
            (flags & macos_ffi::kCGEventFlagMaskCommand) != 0
        }
        _ => return,
    };

    if let Ok(mut pressed) = context.pressed_keys.lock() {
        if is_pressed {
            pressed.insert(key_code);
        } else {
            pressed.remove(&key_code);
        }
    }
}

fn handle_regular_key(context: &EventTapContext, keycode: u16, is_key_down: bool) {
    let key_code = match macos_to_keycode(keycode) {
        Some(k) => k,
        None => return,
    };

    if let Ok(mut pressed) = context.pressed_keys.lock() {
        if is_key_down {
            pressed.insert(key_code);
        } else {
            pressed.remove(&key_code);
        }
    }
}

fn check_combinations(context: &EventTapContext) {
    let pressed = match context.pressed_keys.lock() {
        Ok(p) => p,
        Err(_) => return,
    };

    let now_toggle_matched = context
        .toggle_hotkeys
        .iter()
        .any(|combo| combo.is_subset_of(&pressed));

    if now_toggle_matched && !context.any_toggle_matched.load(Ordering::SeqCst) {
        context.any_toggle_matched.store(true, Ordering::SeqCst);
        debug!("[Hotkey] Toggle hotkey pressed");
        let _ = context.sender.send(HotkeyEvent::TogglePressed);
    } else if !now_toggle_matched && context.any_toggle_matched.load(Ordering::SeqCst) {
        context.any_toggle_matched.store(false, Ordering::SeqCst);
    }

    let now_ptt_matched = context
        .ptt_hotkeys
        .iter()
        .any(|combo| combo.is_subset_of(&pressed));

    let suppress_ptt = context.auto_mode_state.is_active();

    if now_ptt_matched && !context.any_ptt_matched.load(Ordering::SeqCst) {
        context.any_ptt_matched.store(true, Ordering::SeqCst);
        if !suppress_ptt {
            info!("[PTT] Combination MATCHED - key DOWN");
            let _ = context.sender.send(HotkeyEvent::PttPressed);
        } else {
            debug!("[PTT] PTT suppressed (auto mode active)");
        }
    } else if !now_ptt_matched && context.any_ptt_matched.load(Ordering::SeqCst) {
        context.any_ptt_matched.store(false, Ordering::SeqCst);
        if !suppress_ptt {
            info!("[PTT] Combination RELEASED - key UP");
            let _ = context.sender.send(HotkeyEvent::PttReleased);
        }
    }
}

/// FFI bindings for macOS APIs
#[allow(non_upper_case_globals)]
mod macos_ffi {
    use std::ffi::c_void;

    // Types
    pub type CGEventTapProxy = *mut c_void;
    pub type CGEventRef = *mut c_void;
    pub type CGEventType = u32;
    pub type CGEventFlags = u64;
    pub type CFMachPortRef = *mut c_void;
    pub type CFRunLoopSourceRef = *mut c_void;
    pub type CFRunLoopRef = *mut c_void;
    pub type CFAllocatorRef = *const c_void;
    pub type CFStringRef = *const c_void;
    pub type CFTypeRef = *const c_void;

    // Event types
    pub const kCGEventKeyDown: CGEventType = 10;
    pub const kCGEventKeyUp: CGEventType = 11;
    pub const kCGEventFlagsChanged: CGEventType = 12;

    // Event tap locations
    pub const kCGSessionEventTap: u32 = 1;
    pub const kCGHeadInsertEventTap: u32 = 0;
    pub const kCGEventTapOptionListenOnly: u32 = 1;

    // Event field keys
    pub const kCGKeyboardEventKeycode: u32 = 9;

    // Event flags
    pub const kCGEventFlagMaskAlternate: CGEventFlags = 0x00080000;
    pub const kCGEventFlagMaskControl: CGEventFlags = 0x00040000;
    pub const kCGEventFlagMaskShift: CGEventFlags = 0x00020000;
    pub const kCGEventFlagMaskAlphaShift: CGEventFlags = 0x00010000;
    pub const kCGEventFlagMaskCommand: CGEventFlags = 0x00100000;

    // Run loop constants
    pub const kCFRunLoopRunFinished: i32 = 1;

    // Callback type
    pub type CGEventTapCallBack =
        extern "C" fn(CGEventTapProxy, CGEventType, CGEventRef, *mut c_void) -> CGEventRef;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub static kCFRunLoopCommonModes: CFStringRef;
        pub static kCFRunLoopDefaultMode: CFStringRef;

        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
        pub fn CFRunLoopRemoveSource(
            rl: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
        pub fn CFRunLoopRunInMode(
            mode: CFStringRef,
            seconds: f64,
            return_after_source_handled: bool,
        ) -> i32;
        pub fn CFMachPortCreateRunLoopSource(
            allocator: CFAllocatorRef,
            port: CFMachPortRef,
            order: i64,
        ) -> CFRunLoopSourceRef;
        pub fn CFRelease(cf: CFTypeRef);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        pub fn CGEventGetFlags(event: CGEventRef) -> CGEventFlags;
    }

    // Additional CoreFoundation types for AXIsProcessTrustedWithOptions
    pub type CFDictionaryRef = *const c_void;
    pub type CFIndex = isize;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub static kCFBooleanTrue: CFTypeRef;
        pub static kCFTypeDictionaryKeyCallBacks: c_void;
        pub static kCFTypeDictionaryValueCallBacks: c_void;
        pub fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            num_values: CFIndex,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFDictionaryRef;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        /// The key for the options dictionary: kAXTrustedCheckOptionPrompt.
        pub static kAXTrustedCheckOptionPrompt: CFStringRef;

        /// Returns true if the current process has been trusted for Accessibility access.
        /// Safe to call at any time; does not show a system dialog.
        pub fn AXIsProcessTrusted() -> bool;

        /// Returns true if the current process has been trusted for Accessibility access.
        /// When the options dictionary contains kAXTrustedCheckOptionPrompt=true,
        /// macOS shows the system dialog prompting the user to grant access.
        pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    }
}
