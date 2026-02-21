//! Windows clipboard, foreground detection, and paste simulation.
//!
//! Uses Win32 APIs:
//! - Clipboard: `OpenClipboard` / `EmptyClipboard` / `SetClipboardData` / `CloseClipboard`
//! - Foreground: `GetForegroundWindow` / `GetWindowThreadProcessId`
//! - Paste sim: `SendInput` with `INPUT_KEYBOARD` for Ctrl+V

use super::ClipboardPaster;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use tracing::debug;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// The Win32 clipboard format for Unicode text.
const CF_UNICODETEXT: u32 = 13;

pub struct WindowsClipboardPaster;

impl ClipboardPaster for WindowsClipboardPaster {
    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        write_clipboard_text(text)
    }

    fn is_flowstt_foreground(&self) -> bool {
        is_flowstt_foreground_window()
    }

    fn simulate_paste(&self) -> Result<(), String> {
        simulate_ctrl_v()
    }
}

/// Write UTF-16 text to the Windows clipboard.
fn write_clipboard_text(text: &str) -> Result<(), String> {
    unsafe {
        // Encode to UTF-16 with null terminator
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = wide.len() * std::mem::size_of::<u16>();

        // Allocate moveable global memory
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len)
            .map_err(|e| format!("GlobalAlloc failed: {}", e))?;

        // Copy text into the allocated memory
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            return Err("GlobalLock returned null".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, byte_len);
        let _ = GlobalUnlock(hmem);

        // Open clipboard, empty it, set our data, close it
        OpenClipboard(HWND::default()).map_err(|e| format!("OpenClipboard failed: {}", e))?;

        if let Err(e) = EmptyClipboard() {
            let _ = CloseClipboard();
            return Err(format!("EmptyClipboard failed: {}", e));
        }

        // SetClipboardData takes an HANDLE; HGLOBAL and HANDLE share the same
        // underlying representation (*mut c_void).
        let result = SetClipboardData(CF_UNICODETEXT, HANDLE(hmem.0));
        let _ = CloseClipboard();

        result.map_err(|e| format!("SetClipboardData failed: {}", e))?;
        Ok(())
    }
}

/// Check if the foreground window belongs to `flowstt-app.exe`.
fn is_flowstt_foreground_window() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return false;
        }

        // Open the process to query its executable name
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return false,
        };

        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );

        let _ = windows::Win32::Foundation::CloseHandle(handle);

        if ok.is_err() || len == 0 {
            return false;
        }

        let exe_path = OsString::from_wide(&buf[..len as usize]);
        let exe_path_str = exe_path.to_string_lossy();

        // Extract the filename component
        let filename = exe_path_str
            .rsplit('\\')
            .next()
            .unwrap_or("")
            .to_lowercase();

        debug!("[Clipboard] Foreground exe: {}", filename);

        filename == "flowstt-app.exe"
    }
}

/// Simulate Ctrl+V by sending four keyboard events via `SendInput`.
fn simulate_ctrl_v() -> Result<(), String> {
    let inputs = [
        // Ctrl down
        make_key_input(VK_CONTROL, false),
        // V down
        make_key_input(VK_V, false),
        // V up
        make_key_input(VK_V, true),
        // Ctrl up
        make_key_input(VK_CONTROL, true),
    ];

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(format!(
            "SendInput sent {} of {} events",
            sent,
            inputs.len()
        ));
    }
    Ok(())
}

/// Build an `INPUT` struct for a single keyboard event.
fn make_key_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
