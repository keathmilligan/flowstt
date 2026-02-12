//! Windows hotkey backend using Raw Input API.
//!
//! This implementation uses the Windows Raw Input API to monitor global keyboard
//! events even when the application window is not focused. It creates a hidden
//! message-only window to receive WM_INPUT messages.

use super::backend::{HotkeyBackend, HotkeyEvent};
use flowstt_common::KeyCode;
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
    PostThreadMessageW, RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG, PM_REMOVE, WM_INPUT,
    WM_QUIT, WNDCLASSW, WS_OVERLAPPED,
};

/// Raw input keyboard flags
const RI_KEY_BREAK: u16 = 1; // Key up (break) flag
const RI_KEY_E0: u16 = 2; // Extended key flag

/// Windows virtual key codes for PTT keys
/// Note: Raw Input uses generic VK codes (VK_MENU, VK_CONTROL, VK_SHIFT) with
/// the RI_KEY_E0 flag to distinguish left/right keys.
mod vk {
    // Generic modifier keys (used in Raw Input)
    pub const MENU: u16 = 0x12; // VK_MENU (Alt)
    pub const CONTROL: u16 = 0x11; // VK_CONTROL
    pub const SHIFT: u16 = 0x10; // VK_SHIFT

    // Specific modifier keys (for reference, but Raw Input uses generic + E0 flag)
    #[allow(dead_code)]
    pub const RIGHT_ALT: u16 = 0xA5; // VK_RMENU
    #[allow(dead_code)]
    pub const LEFT_ALT: u16 = 0xA4; // VK_LMENU
    #[allow(dead_code)]
    pub const RIGHT_CONTROL: u16 = 0xA3; // VK_RCONTROL
    #[allow(dead_code)]
    pub const LEFT_CONTROL: u16 = 0xA2; // VK_LCONTROL
    #[allow(dead_code)]
    pub const RIGHT_SHIFT: u16 = 0xA1; // VK_RSHIFT
    #[allow(dead_code)]
    pub const LEFT_SHIFT: u16 = 0xA0; // VK_LSHIFT

    pub const CAPS_LOCK: u16 = 0x14; // VK_CAPITAL
    pub const F13: u16 = 0x7C;
    pub const F14: u16 = 0x7D;
    pub const F15: u16 = 0x7E;
    pub const F16: u16 = 0x7F;
    pub const F17: u16 = 0x80;
    pub const F18: u16 = 0x81;
    pub const F19: u16 = 0x82;
    pub const F20: u16 = 0x83;
}

/// Key matching info for Raw Input
/// Returns (vk_code, requires_e0_flag)
/// For modifier keys, Raw Input uses generic VK codes with E0 flag for right-side keys
fn keycode_to_raw_input(key: KeyCode) -> (u16, bool) {
    match key {
        KeyCode::RightAlt => (vk::MENU, true), // VK_MENU + E0 = Right Alt
        KeyCode::LeftAlt => (vk::MENU, false), // VK_MENU without E0 = Left Alt
        KeyCode::RightControl => (vk::CONTROL, true),
        KeyCode::LeftControl => (vk::CONTROL, false),
        KeyCode::RightShift => (vk::SHIFT, true),
        KeyCode::LeftShift => (vk::SHIFT, false),
        KeyCode::CapsLock => (vk::CAPS_LOCK, false),
        KeyCode::F13 => (vk::F13, false),
        KeyCode::F14 => (vk::F14, false),
        KeyCode::F15 => (vk::F15, false),
        KeyCode::F16 => (vk::F16, false),
        KeyCode::F17 => (vk::F17, false),
        KeyCode::F18 => (vk::F18, false),
        KeyCode::F19 => (vk::F19, false),
        KeyCode::F20 => (vk::F20, false),
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
}

impl WindowsHotkeyBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            receiver: None,
            thread_handle: None,
            thread_id: None,
            unavailable_reason: None,
        }
    }
}

impl HotkeyBackend for WindowsHotkeyBackend {
    fn start(&mut self, key: KeyCode) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Hotkey backend already running".to_string());
        }

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let (target_vk, target_requires_e0) = keycode_to_raw_input(key);

        // Channel to receive thread ID from the spawned thread
        let (tid_sender, tid_receiver) = mpsc::channel();

        // Spawn the message loop thread
        let handle = thread::spawn(move || {
            // Get and send our thread ID
            let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
            let _ = tid_sender.send(thread_id);

            info!(
                "[Hotkey] Starting Windows Raw Input message loop for VK {} (E0={})",
                target_vk, target_requires_e0
            );

            if let Err(e) = run_message_loop(running.clone(), sender, target_vk, target_requires_e0)
            {
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

/// Context for hotkey event handling
struct HotkeyContext {
    sender: Sender<HotkeyEvent>,
    target_vk: u16,
    target_requires_e0: bool,
    key_down: bool,
}

/// Run the Windows message loop on this thread
fn run_message_loop(
    running: Arc<AtomicBool>,
    sender: Sender<HotkeyEvent>,
    target_vk: u16,
    target_requires_e0: bool,
) -> Result<(), String> {
    unsafe {
        // Register window class
        let class_name = windows::core::w!("FlowSTT_HotkeyClass");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err("Failed to register window class".to_string());
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
                target_vk,
                target_requires_e0,
                key_down: false,
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

/// Handle a raw input message
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
    let flags = keyboard.Flags;
    let is_key_up = (flags & RI_KEY_BREAK) != 0;
    let is_e0 = (flags & RI_KEY_E0) != 0;

    HOTKEY_CONTEXT.with(|ctx| {
        if let Some(ref mut context) = *ctx.borrow_mut() {
            // Check if this is our target key
            // For modifier keys, we need to match both VK code AND the E0 flag
            let is_target_key = vk_code == context.target_vk && is_e0 == context.target_requires_e0;

            if is_target_key {
                if is_key_up && context.key_down {
                    context.key_down = false;
                    info!("[PTT] Key UP");
                    let _ = context.sender.send(HotkeyEvent::Released);
                } else if !is_key_up && !context.key_down {
                    context.key_down = true;
                    info!("[PTT] Key DOWN");
                    let _ = context.sender.send(HotkeyEvent::Pressed);
                }
                // Note: We don't log repeated key-down events (auto-repeat while held)
            }
        }
    });
}
