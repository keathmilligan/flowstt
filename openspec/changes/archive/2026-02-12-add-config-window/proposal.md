# Change: Add Configuration Window

## Why
Users need a way to configure audio settings (input device, reference/system input, PTT key) without opening the main window. A dedicated configuration window accessible from the system tray provides quick access to these settings.

## What Changes
- Add a new "Configuration" window with controls for: primary input device, reference/system input device, and PTT key selection
- Add a "Settings" menu item to the system tray context menu that opens the configuration window
- The configuration window follows the same visual style as existing windows (rounded borders, dark theme, no native title bar, custom close button)
- Settings take effect immediately when changed (no save button)
- The configuration window can be closed/hidden independently of the main window

## Impact
- Affected specs: `system-tray` (new menu item), new `config-window` capability
- Affected code:
  - `src-tauri/src/tray/mod.rs` and `windows.rs` -- new menu item and window creation
  - `src-tauri/capabilities/default.json` -- add config window to allowed windows
  - New frontend files: `config.html`, `src/config.ts`, `src/config.css`
  - `vite.config.ts` -- add config page to multi-page build
  - Existing IPC commands (`list_all_sources`, `set_sources`, `set_ptt_key`, `get_ptt_status`, `get_status`) are reused; no new backend commands needed
