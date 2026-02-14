# Change: Add Light and Dark Theme Support

## Why
The application currently uses a hardcoded dark color scheme with no ability to switch themes. Users who prefer light interfaces or who work in bright environments have no option to adjust the appearance. Adding theme support with light, dark, and auto (system-follow) modes improves usability and accessibility.

## What Changes
- Introduce a CSS custom property-based theming system that replaces all hardcoded colors across all four CSS files (`styles.css`, `config.css`, `about.css`, `visualization.css`) and the canvas renderers (`renderers.ts`)
- Define a dark theme palette using the existing color scheme as-is
- Define a complementary light theme palette
- Add a theme mode configuration setting (light, dark, auto) to the persisted config
- Default to "auto" mode, which follows the OS `prefers-color-scheme` media query
- Add a theme selector control in the configuration window
- Propagate theme choice to all application windows (main, config, about, visualization)
- Expose theme-related CSS custom properties so canvas renderers read dynamic theme colors at render time

## Impact
- Affected specs: `window-appearance`, `config-window`
- Affected code:
  - `src/styles.css`, `src/config.css`, `src/about.css`, `src/visualization.css` -- replace hardcoded colors with CSS custom properties
  - `src/renderers.ts` -- read theme colors from CSS custom properties instead of hardcoded values
  - `src-common/src/config.rs` -- add `theme_mode` field to `Config`
  - `src/main.ts`, `src/config.ts`, `src/about.ts`, `src/visualization.ts` -- apply theme class on load and respond to config changes
  - `config.html` -- add theme selector UI
  - `visualization.html` -- update inline style colors to use CSS custom properties
