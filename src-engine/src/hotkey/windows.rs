//! Windows hotkey backend using Raw Input API.
//!
//! This implementation uses the Windows Raw Input API to monitor global keyboard
//! events even when the application window is not focused. It creates a hidden
//! message-only window to receive WM_INPUT messages. Supports tracking multiple
//! key combinations simultaneously.

use super::backend::{AutoModeState, HotkeyBackend, HotkeyEvent};
use flowstt_common::{HotkeyCombination, KeyCode};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
    PostThreadMessageW, RegisterClassW, TranslateMessage, UnregisterClassW, HWND_MESSAGE, MSG,
    PM_REMOVE, WM_INPUT, WM_QUIT, WNDCLASSW, WS_OVERLAPPED,
};

/// Raw input keyboard flags
const RI_KEY_BREAK: u16 = 1; // Key up (break) flag
const RI_KEY_E0: u16 = 2; // Extended key flag

/// Windows virtual key codes
mod vk {
    // Generic modifier keys (used in Raw Input)
    pub const MENU: u16 = 0x12; // VK_MENU (Alt)
    pub const CONTROL: u16 = 0x11; // VK_CONTROL
    pub const SHIFT: u16 = 0x10; // VK_SHIFT

    pub const LWIN: u16 = 0x5B; // VK_LWIN
    pub const RWIN: u16 = 0x5C; // VK_RWIN

    pub const CAPS_LOCK: u16 = 0x14; // VK_CAPITAL

    // Function keys
    pub const F1: u16 = 0x70;
    pub const F2: u16 = 0x71;
    pub const F3: u16 = 0x72;
    pub const F4: u16 = 0x73;
    pub const F5: u16 = 0x74;
    pub const F6: u16 = 0x75;
    pub const F7: u16 = 0x76;
    pub const F8: u16 = 0x77;
    pub const F9: u16 = 0x78;
    pub const F10: u16 = 0x79;
    pub const F11: u16 = 0x7A;
    pub const F12: u16 = 0x7B;
    pub const F13: u16 = 0x7C;
    pub const F14: u16 = 0x7D;
    pub const F15: u16 = 0x7E;
    pub const F16: u16 = 0x7F;
    pub const F17: u16 = 0x80;
    pub const F18: u16 = 0x81;
    pub const F19: u16 = 0x82;
    pub const F20: u16 = 0x83;
    pub const F21: u16 = 0x84;
    pub const F22: u16 = 0x85;
    pub const F23: u16 = 0x86;
    pub const F24: u16 = 0x87;

    // Letter keys
    pub const A: u16 = 0x41;
    pub const B: u16 = 0x42;
    pub const C: u16 = 0x43;
    pub const D: u16 = 0x44;
    pub const E: u16 = 0x45;
    pub const F: u16 = 0x46;
    pub const G: u16 = 0x47;
    pub const H: u16 = 0x48;
    pub const I: u16 = 0x49;
    pub const J: u16 = 0x4A;
    pub const K: u16 = 0x4B;
    pub const L: u16 = 0x4C;
    pub const M: u16 = 0x4D;
    pub const N: u16 = 0x4E;
    pub const O: u16 = 0x4F;
    pub const P: u16 = 0x50;
    pub const Q: u16 = 0x51;
    pub const R: u16 = 0x52;
    pub const S: u16 = 0x53;
    pub const T: u16 = 0x54;
    pub const U: u16 = 0x55;
    pub const V: u16 = 0x56;
    pub const W: u16 = 0x57;
    pub const X: u16 = 0x58;
    pub const Y: u16 = 0x59;
    pub const Z: u16 = 0x5A;

    // Digit keys
    pub const DIGIT_0: u16 = 0x30;
    pub const DIGIT_1: u16 = 0x31;
    pub const DIGIT_2: u16 = 0x32;
    pub const DIGIT_3: u16 = 0x33;
    pub const DIGIT_4: u16 = 0x34;
    pub const DIGIT_5: u16 = 0x35;
    pub const DIGIT_6: u16 = 0x36;
    pub const DIGIT_7: u16 = 0x37;
    pub const DIGIT_8: u16 = 0x38;
    pub const DIGIT_9: u16 = 0x39;

    // Navigation keys
    pub const UP: u16 = 0x26;
    pub const DOWN: u16 = 0x28;
    pub const LEFT: u16 = 0x25;
    pub const RIGHT: u16 = 0x27;
    pub const HOME: u16 = 0x24;
    pub const END: u16 = 0x23;
    pub const PRIOR: u16 = 0x21; // VK_PRIOR = Page Up
    pub const NEXT: u16 = 0x22; // VK_NEXT = Page Down
    pub const INSERT: u16 = 0x2D;
    pub const DELETE: u16 = 0x2E;

    // Special keys
    pub const ESCAPE: u16 = 0x1B;
    pub const TAB: u16 = 0x09;
    pub const SPACE: u16 = 0x20;
    pub const RETURN: u16 = 0x0D;
    pub const BACK: u16 = 0x08; // VK_BACK = Backspace
    pub const SNAPSHOT: u16 = 0x2C; // VK_SNAPSHOT = Print Screen
    pub const SCROLL: u16 = 0x91; // VK_SCROLL = Scroll Lock
    pub const PAUSE: u16 = 0x13;

    // Punctuation / symbol keys (US layout VK codes)
    pub const OEM_MINUS: u16 = 0xBD; // - / _
    pub const OEM_PLUS: u16 = 0xBB; // = / +
    pub const OEM_4: u16 = 0xDB; // [ / {
    pub const OEM_6: u16 = 0xDD; // ] / }
    pub const OEM_5: u16 = 0xDC; // \ / |
    pub const OEM_1: u16 = 0xBA; // ; / :
    pub const OEM_7: u16 = 0xDE; // ' / "
    pub const OEM_3: u16 = 0xC0; // ` / ~
    pub const OEM_COMMA: u16 = 0xBC; // ,
    pub const OEM_PERIOD: u16 = 0xBE; // .
    pub const OEM_2: u16 = 0xBF; // / / ?

    // Numpad keys
    pub const NUMPAD0: u16 = 0x60;
    pub const NUMPAD1: u16 = 0x61;
    pub const NUMPAD2: u16 = 0x62;
    pub const NUMPAD3: u16 = 0x63;
    pub const NUMPAD4: u16 = 0x64;
    pub const NUMPAD5: u16 = 0x65;
    pub const NUMPAD6: u16 = 0x66;
    pub const NUMPAD7: u16 = 0x67;
    pub const NUMPAD8: u16 = 0x68;
    pub const NUMPAD9: u16 = 0x69;
    pub const MULTIPLY: u16 = 0x6A;
    pub const ADD: u16 = 0x6B;
    pub const SUBTRACT: u16 = 0x6D;
    pub const DECIMAL: u16 = 0x6E;
    pub const DIVIDE: u16 = 0x6F;
    pub const NUMLOCK: u16 = 0x90;
}

/// Convert a Raw Input VK code, E0 flag, and MakeCode scan code to a KeyCode.
/// Returns None for unmapped keys.
///
/// Shift keys require special handling: unlike Alt and Control, the Raw Input API
/// does NOT set the E0 flag to distinguish Left Shift from Right Shift. Instead,
/// they have distinct MakeCode scan codes: Left Shift = 0x2A, Right Shift = 0x36.
fn raw_input_to_keycode(vk_code: u16, is_e0: bool, make_code: u16) -> Option<KeyCode> {
    match (vk_code, is_e0, make_code) {
        // Modifier keys: generic VK + E0 flag distinguishes left/right for Alt and Control.
        // Shift is different: E0 is never set for Shift — use MakeCode instead.
        (vk::MENU, true, _) => Some(KeyCode::RightAlt),
        (vk::MENU, false, _) => Some(KeyCode::LeftAlt),
        (vk::CONTROL, true, _) => Some(KeyCode::RightControl),
        (vk::CONTROL, false, _) => Some(KeyCode::LeftControl),
        (vk::SHIFT, _, 0x36) => Some(KeyCode::RightShift),
        (vk::SHIFT, _, _) => Some(KeyCode::LeftShift), // MakeCode 0x2A, or unknown fallback
        (vk::CAPS_LOCK, _, _) => Some(KeyCode::CapsLock),
        (vk::LWIN, _, _) => Some(KeyCode::LeftMeta),
        (vk::RWIN, _, _) => Some(KeyCode::RightMeta),
        // Function keys
        (vk::F1, _, _) => Some(KeyCode::F1),
        (vk::F2, _, _) => Some(KeyCode::F2),
        (vk::F3, _, _) => Some(KeyCode::F3),
        (vk::F4, _, _) => Some(KeyCode::F4),
        (vk::F5, _, _) => Some(KeyCode::F5),
        (vk::F6, _, _) => Some(KeyCode::F6),
        (vk::F7, _, _) => Some(KeyCode::F7),
        (vk::F8, _, _) => Some(KeyCode::F8),
        (vk::F9, _, _) => Some(KeyCode::F9),
        (vk::F10, _, _) => Some(KeyCode::F10),
        (vk::F11, _, _) => Some(KeyCode::F11),
        (vk::F12, _, _) => Some(KeyCode::F12),
        (vk::F13, _, _) => Some(KeyCode::F13),
        (vk::F14, _, _) => Some(KeyCode::F14),
        (vk::F15, _, _) => Some(KeyCode::F15),
        (vk::F16, _, _) => Some(KeyCode::F16),
        (vk::F17, _, _) => Some(KeyCode::F17),
        (vk::F18, _, _) => Some(KeyCode::F18),
        (vk::F19, _, _) => Some(KeyCode::F19),
        (vk::F20, _, _) => Some(KeyCode::F20),
        (vk::F21, _, _) => Some(KeyCode::F21),
        (vk::F22, _, _) => Some(KeyCode::F22),
        (vk::F23, _, _) => Some(KeyCode::F23),
        (vk::F24, _, _) => Some(KeyCode::F24),
        // Letter keys
        (vk::A, _, _) => Some(KeyCode::KeyA),
        (vk::B, _, _) => Some(KeyCode::KeyB),
        (vk::C, _, _) => Some(KeyCode::KeyC),
        (vk::D, _, _) => Some(KeyCode::KeyD),
        (vk::E, _, _) => Some(KeyCode::KeyE),
        (vk::F, _, _) => Some(KeyCode::KeyF),
        (vk::G, _, _) => Some(KeyCode::KeyG),
        (vk::H, _, _) => Some(KeyCode::KeyH),
        (vk::I, _, _) => Some(KeyCode::KeyI),
        (vk::J, _, _) => Some(KeyCode::KeyJ),
        (vk::K, _, _) => Some(KeyCode::KeyK),
        (vk::L, _, _) => Some(KeyCode::KeyL),
        (vk::M, _, _) => Some(KeyCode::KeyM),
        (vk::N, _, _) => Some(KeyCode::KeyN),
        (vk::O, _, _) => Some(KeyCode::KeyO),
        (vk::P, _, _) => Some(KeyCode::KeyP),
        (vk::Q, _, _) => Some(KeyCode::KeyQ),
        (vk::R, _, _) => Some(KeyCode::KeyR),
        (vk::S, _, _) => Some(KeyCode::KeyS),
        (vk::T, _, _) => Some(KeyCode::KeyT),
        (vk::U, _, _) => Some(KeyCode::KeyU),
        (vk::V, _, _) => Some(KeyCode::KeyV),
        (vk::W, _, _) => Some(KeyCode::KeyW),
        (vk::X, _, _) => Some(KeyCode::KeyX),
        (vk::Y, _, _) => Some(KeyCode::KeyY),
        (vk::Z, _, _) => Some(KeyCode::KeyZ),
        // Digit keys
        (vk::DIGIT_0, _, _) => Some(KeyCode::Digit0),
        (vk::DIGIT_1, _, _) => Some(KeyCode::Digit1),
        (vk::DIGIT_2, _, _) => Some(KeyCode::Digit2),
        (vk::DIGIT_3, _, _) => Some(KeyCode::Digit3),
        (vk::DIGIT_4, _, _) => Some(KeyCode::Digit4),
        (vk::DIGIT_5, _, _) => Some(KeyCode::Digit5),
        (vk::DIGIT_6, _, _) => Some(KeyCode::Digit6),
        (vk::DIGIT_7, _, _) => Some(KeyCode::Digit7),
        (vk::DIGIT_8, _, _) => Some(KeyCode::Digit8),
        (vk::DIGIT_9, _, _) => Some(KeyCode::Digit9),
        // Navigation keys
        (vk::UP, _, _) => Some(KeyCode::ArrowUp),
        (vk::DOWN, _, _) => Some(KeyCode::ArrowDown),
        (vk::LEFT, _, _) => Some(KeyCode::ArrowLeft),
        (vk::RIGHT, _, _) => Some(KeyCode::ArrowRight),
        (vk::HOME, _, _) => Some(KeyCode::Home),
        (vk::END, _, _) => Some(KeyCode::End),
        (vk::PRIOR, _, _) => Some(KeyCode::PageUp),
        (vk::NEXT, _, _) => Some(KeyCode::PageDown),
        (vk::INSERT, _, _) => Some(KeyCode::Insert),
        (vk::DELETE, _, _) => Some(KeyCode::Delete),
        // Special keys
        (vk::ESCAPE, _, _) => Some(KeyCode::Escape),
        (vk::TAB, _, _) => Some(KeyCode::Tab),
        (vk::SPACE, _, _) => Some(KeyCode::Space),
        (vk::RETURN, _, _) => Some(KeyCode::Enter),
        (vk::BACK, _, _) => Some(KeyCode::Backspace),
        (vk::SNAPSHOT, _, _) => Some(KeyCode::PrintScreen),
        (vk::SCROLL, _, _) => Some(KeyCode::ScrollLock),
        (vk::PAUSE, _, _) => Some(KeyCode::Pause),
        // Punctuation
        (vk::OEM_MINUS, _, _) => Some(KeyCode::Minus),
        (vk::OEM_PLUS, _, _) => Some(KeyCode::Equal),
        (vk::OEM_4, _, _) => Some(KeyCode::BracketLeft),
        (vk::OEM_6, _, _) => Some(KeyCode::BracketRight),
        (vk::OEM_5, _, _) => Some(KeyCode::Backslash),
        (vk::OEM_1, _, _) => Some(KeyCode::Semicolon),
        (vk::OEM_7, _, _) => Some(KeyCode::Quote),
        (vk::OEM_3, _, _) => Some(KeyCode::Backquote),
        (vk::OEM_COMMA, _, _) => Some(KeyCode::Comma),
        (vk::OEM_PERIOD, _, _) => Some(KeyCode::Period),
        (vk::OEM_2, _, _) => Some(KeyCode::Slash),
        // Numpad
        (vk::NUMPAD0, _, _) => Some(KeyCode::Numpad0),
        (vk::NUMPAD1, _, _) => Some(KeyCode::Numpad1),
        (vk::NUMPAD2, _, _) => Some(KeyCode::Numpad2),
        (vk::NUMPAD3, _, _) => Some(KeyCode::Numpad3),
        (vk::NUMPAD4, _, _) => Some(KeyCode::Numpad4),
        (vk::NUMPAD5, _, _) => Some(KeyCode::Numpad5),
        (vk::NUMPAD6, _, _) => Some(KeyCode::Numpad6),
        (vk::NUMPAD7, _, _) => Some(KeyCode::Numpad7),
        (vk::NUMPAD8, _, _) => Some(KeyCode::Numpad8),
        (vk::NUMPAD9, _, _) => Some(KeyCode::Numpad9),
        (vk::MULTIPLY, _, _) => Some(KeyCode::NumpadMultiply),
        (vk::ADD, _, _) => Some(KeyCode::NumpadAdd),
        (vk::SUBTRACT, _, _) => Some(KeyCode::NumpadSubtract),
        (vk::DECIMAL, _, _) => Some(KeyCode::NumpadDecimal),
        (vk::DIVIDE, _, _) => Some(KeyCode::NumpadDivide),
        (vk::NUMLOCK, _, _) => Some(KeyCode::NumLock),
        _ => None,
    }
}

/// Windows hotkey backend using Raw Input API
pub struct WindowsHotkeyBackend {
    /// Whether the backend is currently running
    running: Arc<AtomicBool>,
    /// Channel for receiving hotkey events
    receiver: Option<Receiver<HotkeyEvent>>,
    /// Handle to the message loop thread
    thread_handle: Option<JoinHandle<()>>,
    /// Thread ID for posting quit message
    thread_id: Option<u32>,
    /// Last known unavailability reason
    unavailable_reason: Option<String>,
    /// Auto mode state for PTT suppression (shared with message loop thread)
    auto_mode_state: Arc<AutoModeState>,
}

impl WindowsHotkeyBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            receiver: None,
            thread_handle: None,
            thread_id: None,
            unavailable_reason: None,
            auto_mode_state: AutoModeState::shared(),
        }
    }
}

impl HotkeyBackend for WindowsHotkeyBackend {
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

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let auto_mode_state = self.auto_mode_state.clone();

        // Channel to receive thread ID from the spawned thread
        let (tid_sender, tid_receiver) = mpsc::channel();

        // Spawn the message loop thread
        let handle = thread::spawn(move || {
            // Get and send our thread ID
            let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
            let _ = tid_sender.send(thread_id);

            info!(
                "[Hotkey] Starting Windows Raw Input message loop for {} PTT hotkey(s), {} toggle hotkey(s)",
                ptt_hotkeys.len(),
                toggle_hotkeys.len()
            );

            if let Err(e) = run_message_loop(
                running.clone(),
                sender,
                ptt_hotkeys,
                toggle_hotkeys,
                auto_mode_state,
            ) {
                error!("[Hotkey] Message loop error: {}", e);
            }

            info!("[Hotkey] Message loop thread exiting");
        });

        // Wait for thread ID
        match tid_receiver.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(tid) => {
                self.thread_id = Some(tid);
            }
            Err(_) => {
                self.running.store(false, Ordering::SeqCst);
                return Err("Failed to get message loop thread ID".to_string());
            }
        }

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

        // Post WM_QUIT to the message loop thread
        if let Some(tid) = self.thread_id.take() {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }

        // Wait for thread to exit
        if let Some(handle) = self.thread_handle.take() {
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
        // Windows Raw Input is always available - no special permissions needed
        true
    }

    fn unavailable_reason(&self) -> Option<String> {
        self.unavailable_reason.clone()
    }

    fn set_auto_mode_active(&mut self, active: bool) {
        self.auto_mode_state.set_active(active);
        debug!("[Hotkey] Auto mode PTT suppression: {}", active);
    }
}

impl Drop for WindowsHotkeyBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

// Thread-local context for the window procedure
thread_local! {
    static HOTKEY_CONTEXT: std::cell::RefCell<Option<HotkeyContext>> = const { std::cell::RefCell::new(None) };
}

/// Context for hotkey event handling with combination tracking
struct HotkeyContext {
    sender: Sender<HotkeyEvent>,
    /// All configured PTT hotkey combinations
    ptt_hotkeys: Vec<HotkeyCombination>,
    /// Toggle hotkey combinations
    toggle_hotkeys: Vec<HotkeyCombination>,
    /// Currently pressed keys
    pressed_keys: HashSet<KeyCode>,
    /// Whether any PTT combination is currently matched
    any_ptt_matched: bool,
    /// Whether any toggle combination is currently matched (to avoid repeat)
    any_toggle_matched: bool,
    /// Auto mode state for PTT suppression
    auto_mode_state: Arc<AutoModeState>,
}

/// Run the Windows message loop on this thread
fn run_message_loop(
    running: Arc<AtomicBool>,
    sender: Sender<HotkeyEvent>,
    ptt_hotkeys: Vec<HotkeyCombination>,
    toggle_hotkeys: Vec<HotkeyCombination>,
    auto_mode_state: Arc<AutoModeState>,
) -> Result<(), String> {
    unsafe {
        // Register window class.
        let class_name = windows::core::w!("FlowSTT_HotkeyClass");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            let err = windows::Win32::Foundation::GetLastError();
            if err != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(format!("Failed to register window class (error {:?})", err));
            }
            debug!("[Hotkey] Window class already registered, reusing");
        }

        // Create a message-only window (invisible, just for receiving messages)
        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            windows::core::w!("FlowSTT Hotkey"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            HWND_MESSAGE, // Message-only window
            None,
            None,
            None,
        )
        .map_err(|e| format!("Failed to create message window: {}", e))?;

        // Register for raw keyboard input with RIDEV_INPUTSINK to receive input even when not focused
        let rid = RAWINPUTDEVICE {
            usUsagePage: 0x01, // Generic Desktop Controls
            usUsage: 0x06,     // Keyboard
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        };

        RegisterRawInputDevices(&[rid], size_of::<RAWINPUTDEVICE>() as u32).map_err(|e| {
            let _ = DestroyWindow(hwnd);
            format!("Failed to register raw input device: {}", e)
        })?;

        info!("[Hotkey] Raw input registered, message loop ready");

        // Set up thread-local context
        HOTKEY_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = Some(HotkeyContext {
                sender,
                ptt_hotkeys,
                toggle_hotkeys,
                pressed_keys: HashSet::new(),
                any_ptt_matched: false,
                any_toggle_matched: false,
                auto_mode_state,
            });
        });

        // Message loop using PeekMessageW for non-blocking operation
        let mut msg = MSG::default();

        while running.load(Ordering::SeqCst) {
            // Non-blocking message check
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    debug!("[Hotkey] Received WM_QUIT, exiting loop");
                    running.store(false, Ordering::SeqCst);
                    break;
                }

                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Sleep briefly to avoid busy-waiting
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Cleanup
        HOTKEY_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = None;
        });

        let _ = DestroyWindow(hwnd);
        let _ = UnregisterClassW(class_name, None);

        debug!("[Hotkey] Message loop cleaned up");
    }

    Ok(())
}

/// Window procedure for handling raw input messages
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_INPUT {
        handle_raw_input(HRAWINPUT(lparam.0 as _));
        return LRESULT(0);
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Handle a raw input message - track pressed keys and match combinations
unsafe fn handle_raw_input(hrawinput: HRAWINPUT) {
    // Get the size of the raw input data
    let mut size: u32 = 0;
    GetRawInputData(
        hrawinput,
        RID_INPUT,
        None,
        &mut size,
        size_of::<RAWINPUTHEADER>() as u32,
    );

    if size == 0 {
        return;
    }

    // Allocate buffer and get the data
    let mut buffer = vec![0u8; size as usize];
    let bytes_copied = GetRawInputData(
        hrawinput,
        RID_INPUT,
        Some(buffer.as_mut_ptr() as *mut _),
        &mut size,
        size_of::<RAWINPUTHEADER>() as u32,
    );

    if bytes_copied != size {
        return;
    }

    let raw_input = &*(buffer.as_ptr() as *const RAWINPUT);

    // Only handle keyboard input
    if raw_input.header.dwType != RIM_TYPEKEYBOARD.0 {
        return;
    }

    let keyboard = &raw_input.data.keyboard;
    let vk_code = keyboard.VKey;
    let make_code = keyboard.MakeCode;
    let flags = keyboard.Flags;
    let is_key_up = (flags & RI_KEY_BREAK) != 0;
    let is_e0 = (flags & RI_KEY_E0) != 0;

    // Map VK code to KeyCode
    let key_code = match raw_input_to_keycode(vk_code, is_e0, make_code) {
        Some(k) => {
            debug!(
                "[Hotkey] Raw input: vk=0x{:02X} make=0x{:02X} e0={} up={} → {:?}",
                vk_code, make_code, is_e0, is_key_up, k
            );
            k
        }
        None => {
            debug!(
                "[Hotkey] Raw input: vk=0x{:02X} make=0x{:02X} e0={} up={} → unmapped",
                vk_code, make_code, is_e0, is_key_up
            );
            return; // Unmapped key, ignore
        }
    };

    HOTKEY_CONTEXT.with(|ctx| {
        if let Some(ref mut context) = *ctx.borrow_mut() {
            // Update pressed key set
            if is_key_up {
                context.pressed_keys.remove(&key_code);
            } else {
                context.pressed_keys.insert(key_code);
            }

            // Check if any toggle hotkey is matched
            let now_toggle_matched = context
                .toggle_hotkeys
                .iter()
                .any(|combo| combo.is_subset_of(&context.pressed_keys));

            // Toggle on press only (not release), avoid repeat
            if now_toggle_matched && !context.any_toggle_matched {
                context.any_toggle_matched = true;
                info!("[Hotkey] Toggle hotkey pressed");
                let _ = context.sender.send(HotkeyEvent::TogglePressed);
            } else if !now_toggle_matched && context.any_toggle_matched {
                context.any_toggle_matched = false;
            }

            // Check if any PTT combination is now matched
            let now_ptt_matched = context
                .ptt_hotkeys
                .iter()
                .any(|combo| combo.is_subset_of(&context.pressed_keys));

            // Emit PTT events on state transitions (unless suppressed)
            let suppress_ptt = context.auto_mode_state.is_active();

            if now_ptt_matched && !context.any_ptt_matched {
                context.any_ptt_matched = true;
                if !suppress_ptt {
                    info!("[PTT] Combination MATCHED - key DOWN");
                    let _ = context.sender.send(HotkeyEvent::PttPressed);
                } else {
                    debug!("[PTT] PTT suppressed (auto mode active)");
                }
            } else if !now_ptt_matched && context.any_ptt_matched {
                context.any_ptt_matched = false;
                if !suppress_ptt {
                    info!("[PTT] Combination RELEASED - key UP");
                    let _ = context.sender.send(HotkeyEvent::PttReleased);
                }
            }
        }
    });
}
