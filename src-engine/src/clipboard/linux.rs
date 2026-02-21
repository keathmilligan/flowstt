//! Linux clipboard, foreground detection, and paste simulation.
//!
//! Uses system CLI tools with graceful fallback:
//! - Clipboard: `xclip` (X11) or `wl-copy` (Wayland)
//! - Foreground: `xdotool getactivewindow getwindowpid` (X11) or best-effort
//! - Paste: `xdotool key ctrl+v` (X11) or `wtype -M ctrl -k v` (Wayland)

use super::ClipboardPaster;
use std::process::Command;
use tracing::{debug, warn};

pub struct LinuxClipboardPaster;

impl ClipboardPaster for LinuxClipboardPaster {
    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        // Try wl-copy first (Wayland), then xclip (X11).
        if is_wayland() {
            run_clipboard_write("wl-copy", &["--"], text)
        } else {
            run_clipboard_write("xclip", &["-selection", "clipboard"], text)
        }
    }

    fn is_flowstt_foreground(&self) -> bool {
        if is_wayland() {
            // Wayland does not expose a reliable way to query the focused
            // window from an unprivileged process. Default to allowing paste.
            false
        } else {
            is_flowstt_foreground_x11()
        }
    }

    fn simulate_paste(&self) -> Result<(), String> {
        if is_wayland() {
            let status = Command::new("wtype")
                .args(["-M", "ctrl", "-k", "v", "-m", "ctrl"])
                .status()
                .map_err(|e| format!("Failed to run wtype: {} (is wtype installed?)", e))?;

            if !status.success() {
                return Err(format!("wtype exited with status {}", status));
            }
            Ok(())
        } else {
            let status = Command::new("xdotool")
                .args(["key", "ctrl+v"])
                .status()
                .map_err(|e| format!("Failed to run xdotool: {} (is xdotool installed?)", e))?;

            if !status.success() {
                return Err(format!("xdotool exited with status {}", status));
            }
            Ok(())
        }
    }
}

/// Detect whether we're running under Wayland.
fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Write text to clipboard via a subprocess that reads stdin.
fn run_clipboard_write(cmd: &str, args: &[&str], text: &str) -> Result<(), String> {
    use std::io::Write;

    let mut child = Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {} (is it installed?)", cmd, e))?;

    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to {} stdin: {}", cmd, e))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for {}: {}", cmd, e))?;
    if !status.success() {
        return Err(format!("{} exited with status {}", cmd, status));
    }
    Ok(())
}

/// Check if the focused X11 window belongs to flowstt-app.
fn is_flowstt_foreground_x11() -> bool {
    // Get the PID of the active window
    let output = match Command::new("xdotool")
        .args(["getactivewindow", "getwindowpid"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            warn!("[Clipboard] xdotool not available: {}", e);
            return false;
        }
    };

    if !output.status.success() {
        return false;
    }

    let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let pid: u32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Read /proc/<pid>/exe symlink to get the executable path
    let exe_path = match std::fs::read_link(format!("/proc/{}/exe", pid)) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let filename = exe_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    debug!("[Clipboard] Foreground exe: {}", filename);

    filename == "flowstt-app"
}
