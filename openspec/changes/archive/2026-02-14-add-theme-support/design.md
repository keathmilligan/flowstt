## Context
The application has four separate CSS files with ~100+ hardcoded color values and no shared theming infrastructure. Canvas-based renderers in `renderers.ts` also use hardcoded colors with a few CSS custom property reads. The Rust-side spectrogram colormap is hardcoded. A config system exists in `src-common/src/config.rs` but has no theme-related fields. There is no frontend-side persistent storage or Tauri store plugin.

## Goals / Non-Goals

### Goals
- Centralize all colors into CSS custom properties so themes can be switched by changing a single data attribute
- Create a light theme with complementary colors that maintain the same visual hierarchy and contrast ratios
- Preserve the current dark theme exactly as-is
- Let users choose between light, dark, and auto (OS-follow) modes
- Persist the theme choice in the existing config file
- Apply the theme consistently across all four windows

### Non-Goals
- Custom user-defined color palettes or accent color pickers
- Per-window theme overrides
- Theming the Rust-side spectrogram colormap (it uses a fixed scientific heatmap that works on both light and dark backgrounds)
- Theming the system tray icon

## Decisions

### CSS Custom Properties on `:root` with `data-theme` attribute
- **Decision**: Define all color tokens as CSS custom properties under `[data-theme="dark"]` and `[data-theme="light"]` selectors. A shared `theme.css` file holds all variable definitions and is imported by each window's CSS file.
- **Alternatives considered**:
  - CSS classes (`.dark`, `.light`): data attributes are more semantic and avoid class name conflicts
  - Separate CSS files per theme: harder to maintain, requires runtime CSS swapping
  - Tailwind dark mode: project doesn't use Tailwind

### Theme application via `<html data-theme="...">` attribute
- **Decision**: Set the `data-theme` attribute on `<html>` in each window. A shared `theme.ts` module reads the config, listens for OS preference changes (for auto mode), and sets the attribute.
- **Why**: Simple, no framework needed, works with the existing vanilla TS architecture. All CSS rules use the custom properties, so changing the attribute instantly re-themes the window.

### Auto mode implementation
- **Decision**: Use `window.matchMedia('(prefers-color-scheme: dark)')` to detect OS preference. Register a listener for changes. When mode is "auto", follow the media query result; when "light" or "dark", ignore the media query.
- **Alternatives considered**:
  - Tauri OS theme API (`window.theme()`): would require Tauri plugin setup; `matchMedia` works everywhere and is simpler.

### Config persistence via existing config.json
- **Decision**: Add a `theme_mode` field (`"auto"` | `"light"` | `"dark"`) to the `Config` struct in `src-common/src/config.rs`, defaulting to `"auto"`. The frontend communicates theme changes via a new IPC command.
- **Why**: Reuses the existing persistence mechanism. No new dependencies needed.

### Shared theme.css file
- **Decision**: Create a single `src/theme.css` that defines all CSS custom property values for both themes. Each window's CSS file imports it via `@import './theme.css'`. Existing hardcoded values in `styles.css`, `config.css`, `about.css`, and `visualization.css` are replaced with `var(--token-name)` references.
- **Why**: Eliminates duplication across the four CSS files. Single source of truth for color values.

### Canvas renderer theming
- **Decision**: Extend the existing pattern in `renderers.ts` where it reads CSS custom properties via `getComputedStyle()`. Add new custom properties for all currently hardcoded canvas colors (speech activity colors, threshold lines, etc.). Renderers re-read properties on each frame so theme changes take effect immediately.
- **Why**: Consistent with the existing partial implementation. No architecture change needed.

## Risks / Trade-offs

- **Risk**: Large number of hardcoded colors to extract and tokenize (~100+) increases chance of missed values.
  - Mitigation: Systematic audit of each CSS file. Visual comparison testing of both themes.
- **Risk**: Canvas renderers re-reading CSS properties every frame could have performance impact.
  - Mitigation: Already done for 7 properties without issue. Cache property reads and only refresh on theme change events.
- **Risk**: Light theme colors may not provide sufficient contrast on all elements.
  - Mitigation: Follow WCAG AA contrast guidelines. Test against common elements.

## Open Questions
- None at this time. The spectrogram colormap in Rust is intentionally excluded from theming as its scientific heatmap palette is theme-agnostic.
