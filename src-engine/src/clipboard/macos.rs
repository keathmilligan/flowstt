//! macOS clipboard, foreground detection, and paste simulation.
//!
//! Uses:
//! - `NSPasteboard` for clipboard write
//! - `NSWorkspace.shared.frontmostApplication` for foreground detection
//! - `CGEvent` for Cmd+V paste simulation

use super::ClipboardPaster;
use std::process::Command;
use tracing::debug;

pub struct MacOSClipboardPaster;

impl ClipboardPaster for MacOSClipboardPaster {
    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        // Use pbcopy for simplicity -- it is always available on macOS.
        let mut child = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn pbcopy: {}", e))?;

        use std::io::Write;
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to pbcopy stdin: {}", e))?;
        }

        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for pbcopy: {}", e))?;
        if !status.success() {
            return Err(format!("pbcopy exited with status {}", status));
        }
        Ok(())
    }

    fn is_flowstt_foreground(&self) -> bool {
        // Use osascript to query the frontmost application name.
        // This avoids needing unsafe Obj-C bindings for this single check.
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to get name of first process whose frontmost is true"#)
            .output();

        match output {
            Ok(out) => {
                let name = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                debug!("[Clipboard] Foreground app: {}", name);
                name.contains("flowstt")
            }
            Err(_) => false, // Default to allowing paste
        }
    }

    fn simulate_paste(&self) -> Result<(), String> {
        // Use osascript to send Cmd+V keystroke.
        // This requires Accessibility permission (which FlowSTT already needs
        // for global hotkey capture).
        let status = Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
            .status()
            .map_err(|e| format!("Failed to run osascript for paste: {}", e))?;

        if !status.success() {
            return Err(format!("osascript paste exited with status {}", status));
        }
        Ok(())
    }
}
