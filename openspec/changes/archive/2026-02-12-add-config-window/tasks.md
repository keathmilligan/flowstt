## 1. Backend: Tray Menu and Window Creation
- [x] 1.1 Add "Settings" menu item ID and label to `src-tauri/src/tray/mod.rs`
- [x] 1.2 Add "Settings" `MenuItem` to the tray context menu in `src-tauri/src/tray/windows.rs` (between "Show" and "About")
- [x] 1.3 Add `show_config_window()` function in `src-tauri/src/tray/windows.rs` following the About window pattern (label: `"config"`, URL: `config.html`, `decorations: false`, `transparent: true`, `shadow: false`, `skip_taskbar: true`, non-resizable)
- [x] 1.4 Handle `menu_ids::SETTINGS` in `handle_menu_event` to call `show_config_window()`
- [x] 1.5 Add `"config"` to the windows list in `src-tauri/capabilities/default.json`

## 2. Frontend: Config Page Scaffolding
- [x] 2.1 Create `config.html` at project root (following `about.html` pattern) with dropdowns for primary input, reference input, and PTT key, plus a close button
- [x] 2.2 Create `src/config.css` with shared dark theme, rounded border, and close button styles (following `about.css` pattern)
- [x] 2.3 Create `src/config.ts` with initialization logic: close button handler, context menu suppression, keyboard shortcut suppression
- [x] 2.4 Add `config: "config.html"` entry to `vite.config.ts` rollup input

## 3. Frontend: Config Window Logic
- [x] 3.1 On window open, call `invoke("list_all_sources")` to populate both device dropdowns and `invoke("get_ptt_status")` to populate PTT key dropdown with current selection
- [x] 3.2 Call `invoke("get_status")` to determine current source selections and pre-select the correct options
- [x] 3.3 Wire primary input dropdown `change` handler to call `invoke("set_sources", { source1Id, source2Id })` with updated source1 and current source2
- [x] 3.4 Wire reference input dropdown `change` handler to call `invoke("set_sources", { source1Id, source2Id })` with current source1 and updated source2
- [x] 3.5 Wire PTT key dropdown `change` handler to call `invoke("set_ptt_key", { key })`

## 4. Verification
- [x] 4.1 Run `pnpm build` (Vite) to confirm the config page is included in the multi-page build output
- [x] 4.2 Run `cargo build` to confirm the Rust changes compile without errors
