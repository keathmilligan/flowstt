//! macOS hotkey backend using CGEventTap.
//!
//! This implementation uses the Core Graphics Event Tap API to monitor
//! global keyboard events. It requires Accessibility permission to function.

use super::backend::{AutoModeState, HotkeyBackend, HotkeyEvent};
use flowstt_common::{HotkeyCombination, KeyCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info};

// macOS virtual key codes
mod keycode {
    pub const RIGHT_OPTION: u16 = 0x3D; // 61
    pub const LEFT_OPTION: u16 = 0x3A; // 58
    pub const RIGHT_CONTROL: u16 = 0x3E; // 62
    pub const LEFT_CONTROL: u16 = 0x3B; // 59
    pub const RIGHT_SHIFT: u16 = 0x3C; // 60
    pub const LEFT_SHIFT: u16 = 0x38; // 56
    pub const CAPS_LOCK: u16 = 0x39; // 57
    pub const F13: u16 = 0x69; // 105
    pub const F14: u16 = 0x6B; // 107
    pub const F15: u16 = 0x71; // 113
    pub const F16: u16 = 0x6A; // 106
    pub const F17: u16 = 0x40; // 64
    pub const F18: u16 = 0x4F; // 79
    pub const F19: u16 = 0x50; // 80
    pub const F20: u16 = 0x5A; // 90
}

/// Convert KeyCode to macOS virtual key code.
/// Returns 0xFFFF for keys not yet mapped on macOS.
fn keycode_to_macos(key: KeyCode) -> u16 {
    match key {
        KeyCode::RightAlt => keycode::RIGHT_OPTION,
        KeyCode::LeftAlt => keycode::LEFT_OPTION,
        KeyCode::RightControl => keycode::RIGHT_CONTROL,
        KeyCode::LeftControl => keycode::LEFT_CONTROL,
        KeyCode::RightShift => keycode::RIGHT_SHIFT,
        KeyCode::LeftShift => keycode::LEFT_SHIFT,
        KeyCode::CapsLock => keycode::CAPS_LOCK,
        KeyCode::F13 => keycode::F13,
        KeyCode::F14 => keycode::F14,
        KeyCode::F15 => keycode::F15,
        KeyCode::F16 => keycode::F16,
        KeyCode::F17 => keycode::F17,
        KeyCode::F18 => keycode::F18,
        KeyCode::F19 => keycode::F19,
        KeyCode::F20 => keycode::F20,
        // TODO: Map remaining keys when macOS backend gets full combination support
        _ => 0xFFFF,
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

    /// Check if we have Accessibility permission
    fn check_accessibility_permission() -> bool {
        // Use the ApplicationServices framework to check permission
        // AXIsProcessTrustedWithOptions with prompt option
        unsafe {
            let trusted = macos_ffi::AXIsProcessTrusted();
            trusted
        }
    }

    /// Request accessibility permission (shows system dialog)
    fn request_accessibility_permission() -> bool {
        unsafe {
            // Create options dictionary with prompt key set to true
            let options = macos_ffi::CFDictionaryCreate(
                std::ptr::null(),
                &macos_ffi::kAXTrustedCheckOptionPrompt as *const _
                    as *const *const std::ffi::c_void,
                &macos_ffi::kCFBooleanTrue as *const _ as *const *const std::ffi::c_void,
                1,
                &macos_ffi::kCFTypeDictionaryKeyCallBacks,
                &macos_ffi::kCFTypeDictionaryValueCallBacks,
            );

            let trusted = macos_ffi::AXIsProcessTrustedWithOptions(options);

            if !options.is_null() {
                macos_ffi::CFRelease(options as *const std::ffi::c_void);
            }

            trusted
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

        // macOS backend: for now, use the first key of the first combination
        // (full combination support to be implemented later)
        let ptt_key = ptt_hotkeys
            .first()
            .and_then(|h| h.keys.first().copied())
            .unwrap_or_default();

        let toggle_key = toggle_hotkeys.first().and_then(|h| h.keys.first().copied());

        // Check accessibility permission, prompt if not granted
        if !Self::check_accessibility_permission() {
            info!("[Hotkey] Accessibility permission not granted, requesting...");
            // This will show the system permission dialog
            let granted = Self::request_accessibility_permission();
            if !granted {
                let msg = "Push-to-Talk requires Accessibility permission to detect hotkeys. Grant permission in System Settings > Privacy & Security > Accessibility, then restart FlowSTT.".to_string();
                self.unavailable_reason = Some(msg.clone());
                return Err(msg);
            }
        }

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let ptt_keycode = keycode_to_macos(ptt_key);
        let toggle_keycode = toggle_key.map(keycode_to_macos);
        let auto_mode_state = self.auto_mode_state.clone();

        // Spawn the event tap thread
        let handle = thread::spawn(move || {
            info!(
                "[Hotkey] Starting macOS event tap for PTT key {}, toggle key {:?}",
                ptt_keycode, toggle_keycode
            );

            if let Err(e) = run_event_tap(
                running.clone(),
                sender,
                ptt_keycode,
                toggle_keycode,
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
    ptt_keycode: u16,
    toggle_keycode: Option<u16>,
    auto_mode_state: Arc<AutoModeState>,
) -> Result<(), String> {
    unsafe {
        // Create a mach port for the event tap
        let event_mask = (1 << macos_ffi::kCGEventKeyDown)
            | (1 << macos_ffi::kCGEventKeyUp)
            | (1 << macos_ffi::kCGEventFlagsChanged);

        // Store context for the callback
        let context = Box::new(EventTapContext {
            sender,
            ptt_keycode,
            toggle_keycode,
            ptt_key_down: AtomicBool::new(false),
            toggle_key_down: AtomicBool::new(false),
            auto_mode_state,
        });
        let context_ptr = Box::into_raw(context);

        let tap = macos_ffi::CGEventTapCreate(
            macos_ffi::kCGSessionEventTap,
            macos_ffi::kCGHeadInsertEventTap,
            macos_ffi::kCGEventTapOptionListenOnly, // Passive mode
            event_mask,
            event_tap_callback,
            context_ptr as *mut std::ffi::c_void,
        );

        if tap.is_null() {
            let _ = Box::from_raw(context_ptr); // Clean up
            return Err("Failed to create event tap. Check Accessibility permissions.".to_string());
        }

        // Create a run loop source
        let run_loop_source = macos_ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);

        if run_loop_source.is_null() {
            macos_ffi::CFRelease(tap as *const std::ffi::c_void);
            let _ = Box::from_raw(context_ptr);
            return Err("Failed to create run loop source".to_string());
        }

        // Get the current run loop and add the source
        let run_loop = macos_ffi::CFRunLoopGetCurrent();
        macos_ffi::CFRunLoopAddSource(run_loop, run_loop_source, macos_ffi::kCFRunLoopCommonModes);

        // Enable the event tap
        macos_ffi::CGEventTapEnable(tap, true);

        debug!("[Hotkey] Event tap created and enabled");

        // Run the loop until stopped
        while running.load(Ordering::SeqCst) {
            // Run for a short interval then check if we should stop
            let result = macos_ffi::CFRunLoopRunInMode(
                macos_ffi::kCFRunLoopDefaultMode,
                0.1, // 100ms timeout
                true,
            );

            if result == macos_ffi::kCFRunLoopRunFinished {
                break;
            }
        }

        // Cleanup
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

/// Context passed to the event tap callback
struct EventTapContext {
    sender: Sender<HotkeyEvent>,
    ptt_keycode: u16,
    toggle_keycode: Option<u16>,
    ptt_key_down: AtomicBool,
    toggle_key_down: AtomicBool,
    auto_mode_state: Arc<AutoModeState>,
}

/// CGEventTap callback function
extern "C" fn event_tap_callback(
    _proxy: macos_ffi::CGEventTapProxy,
    event_type: macos_ffi::CGEventType,
    event: macos_ffi::CGEventRef,
    user_info: *mut std::ffi::c_void,
) -> macos_ffi::CGEventRef {
    let context = unsafe { &*(user_info as *const EventTapContext) };

    // Handle flags changed events (for modifier keys like Option, Shift, Control)
    if event_type == macos_ffi::kCGEventFlagsChanged {
        let keycode = unsafe {
            macos_ffi::CGEventGetIntegerValueField(event, macos_ffi::kCGKeyboardEventKeycode)
        } as u16;

        // Check if this is a toggle key
        if let Some(toggle_kc) = context.toggle_keycode {
            if keycode == toggle_kc {
                let was_down = context.toggle_key_down.swap(true, Ordering::SeqCst);
                if !was_down {
                    debug!("[Hotkey] Toggle key pressed (flags changed)");
                    let _ = context.sender.send(HotkeyEvent::TogglePressed);
                }
                return event;
            }
        }

        // Check PTT key
        if keycode == context.ptt_keycode {
            // Check if the modifier is pressed by looking at the flags
            let flags = unsafe { macos_ffi::CGEventGetFlags(event) };
            let is_pressed = match context.ptt_keycode {
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
                _ => false,
            };

            let was_down = context.ptt_key_down.load(Ordering::SeqCst);

            if is_pressed && !was_down {
                context.ptt_key_down.store(true, Ordering::SeqCst);
                // Suppress PTT if auto mode is active
                if !context.auto_mode_state.is_active() {
                    debug!("[Hotkey] PTT key pressed (flags changed)");
                    let _ = context.sender.send(HotkeyEvent::PttPressed);
                }
            } else if !is_pressed && was_down {
                context.ptt_key_down.store(false, Ordering::SeqCst);
                if !context.auto_mode_state.is_active() {
                    debug!("[Hotkey] PTT key released (flags changed)");
                    let _ = context.sender.send(HotkeyEvent::PttReleased);
                }
            }
        }
    }
    // Handle regular key events (for function keys)
    else if event_type == macos_ffi::kCGEventKeyDown || event_type == macos_ffi::kCGEventKeyUp {
        let keycode = unsafe {
            macos_ffi::CGEventGetIntegerValueField(event, macos_ffi::kCGKeyboardEventKeycode)
        } as u16;

        // Check toggle key
        if let Some(toggle_kc) = context.toggle_keycode {
            if keycode == toggle_kc && event_type == macos_ffi::kCGEventKeyDown {
                let was_down = context.toggle_key_down.swap(true, Ordering::SeqCst);
                if !was_down {
                    debug!("[Hotkey] Toggle key pressed (key down)");
                    let _ = context.sender.send(HotkeyEvent::TogglePressed);
                }
            } else if keycode == toggle_kc && event_type == macos_ffi::kCGEventKeyUp {
                context.toggle_key_down.store(false, Ordering::SeqCst);
            }
        }

        // Check PTT key
        if keycode == context.ptt_keycode {
            if event_type == macos_ffi::kCGEventKeyDown {
                let was_down = context.ptt_key_down.swap(true, Ordering::SeqCst);
                if !was_down && !context.auto_mode_state.is_active() {
                    debug!("[Hotkey] PTT key pressed (key down)");
                    let _ = context.sender.send(HotkeyEvent::PttPressed);
                }
            } else {
                context.ptt_key_down.store(false, Ordering::SeqCst);
                if !context.auto_mode_state.is_active() {
                    debug!("[Hotkey] PTT key released (key up)");
                    let _ = context.sender.send(HotkeyEvent::PttReleased);
                }
            }
        }
    }

    event
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
    pub type CFDictionaryRef = *mut c_void;
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

    // Run loop constants
    pub const kCFRunLoopRunFinished: i32 = 1;

    // Callback type
    pub type CGEventTapCallBack =
        extern "C" fn(CGEventTapProxy, CGEventType, CGEventRef, *mut c_void) -> CGEventRef;

    // CF Types
    #[repr(C)]
    pub struct CFDictionaryKeyCallBacks {
        _data: [u8; 0],
    }

    #[repr(C)]
    pub struct CFDictionaryValueCallBacks {
        _data: [u8; 0],
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub static kCFRunLoopCommonModes: CFStringRef;
        pub static kCFRunLoopDefaultMode: CFStringRef;
        pub static kCFBooleanTrue: CFTypeRef;
        pub static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
        pub static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;

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
        pub fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: i64,
            key_callbacks: *const CFDictionaryKeyCallBacks,
            value_callbacks: *const CFDictionaryValueCallBacks,
        ) -> CFDictionaryRef;
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

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        pub static kAXTrustedCheckOptionPrompt: CFStringRef;
        pub fn AXIsProcessTrusted() -> bool;
        pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    }
}
