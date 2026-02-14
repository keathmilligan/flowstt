## 1. Backend: Config and IPC

- [x] 1.1 Add `theme_mode` field (`"auto"` | `"light"` | `"dark"`, default `"auto"`) to `Config` struct in `src-common/src/config.rs`
- [x] 1.2 Add serialization/deserialization for the new field with backward-compatible defaults
- [x] 1.3 Add a Tauri IPC command to get and set `theme_mode` (e.g., `get_theme_mode`, `set_theme_mode`)
- [x] 1.4 Emit a Tauri event when theme mode changes so all windows can react

## 2. Shared Theme Stylesheet

- [x] 2.1 Audit all four CSS files and `renderers.ts` to catalog every hardcoded color value; create a color token mapping
- [x] 2.2 Create `src/theme.css` defining all CSS custom properties under `[data-theme="dark"]` using the existing color values
- [x] 2.3 Define `[data-theme="light"]` in `src/theme.css` with a complementary light palette
- [x] 2.4 Add `@import './theme.css'` to `styles.css`, `config.css`, `about.css`, and `visualization.css`

## 3. Migrate Hardcoded Colors to CSS Variables

- [x] 3.1 Replace all hardcoded color values in `src/styles.css` with `var(--token-name)` references
- [x] 3.2 Replace all hardcoded color values in `src/config.css` with `var(--token-name)` references
- [x] 3.3 Replace all hardcoded color values in `src/about.css` with `var(--token-name)` references
- [x] 3.4 Replace all hardcoded color values in `src/visualization.css` with `var(--token-name)` references
- [x] 3.5 Replace hardcoded inline styles in `visualization.html` with CSS custom properties
- [x] 3.6 Add CSS custom properties for all canvas renderer colors and update `renderers.ts` to read them via `getComputedStyle()`

## 4. Theme Application Module

- [x] 4.1 Create `src/theme.ts` with functions: `initTheme()` (reads config, sets `data-theme` on `<html>`), `setThemeMode(mode)`, and a `matchMedia` listener for auto mode
- [x] 4.2 Import and call `initTheme()` in `src/main.ts`, `src/config.ts`, `src/about.ts`, and `src/visualization.ts` before first paint
- [x] 4.3 Listen for the Tauri theme-change event in each window to update `data-theme` when changed from the config window

## 5. Config Window UI

- [x] 5.1 Add a theme mode selector (dropdown or radio group) to `config.html` with options: Auto, Light, Dark
- [x] 5.2 Style the theme selector in `config.css` using the new CSS custom properties
- [x] 5.3 Wire the selector in `src/config.ts` to call the `set_theme_mode` IPC command on change
- [x] 5.4 Pre-select the current theme mode when the config window opens

## 6. Verification

- [ ] 6.1 Visual test: confirm dark theme matches the current appearance exactly
- [ ] 6.2 Visual test: confirm light theme renders all elements with readable contrast
- [ ] 6.3 Functional test: switching between light, dark, and auto modes updates all open windows
- [ ] 6.4 Functional test: auto mode follows OS preference and responds to OS changes in real time
- [ ] 6.5 Persistence test: theme choice survives application restart
- [x] 6.6 Build test: `pnpm build` and `cargo build` complete without errors
