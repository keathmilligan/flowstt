# window-appearance Specification

## Purpose
TBD - created by archiving change update-window-appearance. Update Purpose after archive.
## Requirements
### Requirement: Gradient Background
The application SHALL display a gradient background across the entire window, using theme-appropriate colors. In dark mode, the gradient uses dark gray tones. In light mode, the gradient uses light neutral tones.

#### Scenario: Background renders on launch
- **WHEN** the application window opens
- **THEN** the background displays a smooth gradient appropriate to the active theme

#### Scenario: Background adapts to theme change
- **WHEN** the user switches the theme mode
- **THEN** the gradient background transitions to the new theme's color palette

### Requirement: Fixed Window Size
The main application window SHALL be resizable with a default compact size of 900x340 pixels and a minimum size of 600x300 pixels. All content and components SHALL resize within the window, maintaining their position and margins relative to the window edges.

#### Scenario: Default window size on launch
- **WHEN** the application window opens for the first time
- **THEN** the window displays at 900x340 pixels

#### Scenario: User resizes main window
- **WHEN** the user drags the edges or corners of the main window
- **THEN** the window resizes freely in both dimensions and all content adapts to the new size

#### Scenario: Minimum size enforced
- **WHEN** the user attempts to resize the main window below 600x300 pixels
- **THEN** the window stops resizing and maintains at least 600x300 dimensions

#### Scenario: Content maintains relative layout on resize
- **WHEN** the main window is resized
- **THEN** the header, controls bar, and transcription area maintain their relative positions and margins to the window edges
- **AND** the transcription area expands or contracts to fill available space

### Requirement: No Title Bar
The application window SHALL display without a native title bar (window decorations disabled).

#### Scenario: Window renders without decorations
- **WHEN** the application window opens
- **THEN** no native title bar or window frame decorations are visible

### Requirement: Custom Drag Region
The application window background SHALL be draggable to allow window repositioning without a native title bar. Interactive elements (buttons, inputs, selects, toggles, canvases) SHALL be excluded from the drag region.

#### Scenario: User drags window via background
- **WHEN** the user clicks and drags on any non-interactive background area
- **THEN** the window moves with the cursor to reposition on screen

#### Scenario: Interactive elements remain functional
- **WHEN** the user clicks on a button, input, select, toggle, or canvas
- **THEN** the element receives the click event normally without initiating window drag

#### Scenario: Windows platform support
- **WHEN** the application runs on Windows
- **THEN** the `-webkit-app-region: drag` CSS property enables native window dragging

### Requirement: Visualization Window
The system SHALL provide a separate resizable window for displaying audio visualizations (waveform, spectrogram, speech activity graph).

#### Scenario: Visualization window opens on demand
- **WHEN** the user double-clicks the mini waveform in the main window header
- **THEN** the visualization window opens displaying all three visualizations

#### Scenario: Visualization window is resizable
- **WHEN** the user drags the edges or corners of the visualization window
- **THEN** the window resizes freely in both dimensions

#### Scenario: Visualization window minimum size
- **WHEN** the user attempts to resize the visualization window below 800x600 pixels
- **THEN** the window stops resizing and maintains at least 800x600 dimensions

#### Scenario: Visualization window closed by default
- **WHEN** the application starts
- **THEN** only the main window is visible; the visualization window is not open

#### Scenario: Visualization window can be closed independently
- **WHEN** the user closes the visualization window
- **THEN** the main window remains open and functional

#### Scenario: Visualization window has no title bar
- **WHEN** the visualization window opens
- **THEN** it displays without native title bar decorations, matching the main window style

#### Scenario: Opening already-open visualization window focuses it
- **WHEN** the user double-clicks the mini waveform while the visualization window is already open
- **THEN** the existing visualization window is focused instead of opening a new window

### Requirement: Mini Waveform Display
The main window SHALL display a miniature real-time waveform visualization in the header area next to the application logo. The mini waveform SHALL only be visible when audio recording or capture is active.

#### Scenario: Mini waveform position and alignment
- **WHEN** the main window renders and audio capture is active
- **THEN** the mini waveform appears immediately to the right of the logo, vertically centered with the logo

#### Scenario: Mini waveform size
- **WHEN** the mini waveform renders
- **THEN** its height is proportional to the logo height and its width provides adequate visualization (~120 pixels wide)

#### Scenario: Mini waveform appearance
- **WHEN** the mini waveform renders
- **THEN** it displays a gray waveform line on a transparent background with no scale, axis labels, or grid lines

#### Scenario: Mini waveform animates in real-time
- **WHEN** audio monitoring or transcription is active
- **THEN** the mini waveform displays scrolling audio amplitude in real-time, matching the time window of the full waveform (~80ms)

#### Scenario: Mini waveform hidden when idle
- **WHEN** audio capture is not active (no PTT key held, no automatic speech capture in progress)
- **THEN** the mini waveform is not visible (hidden via CSS display none)

#### Scenario: Mini waveform becomes visible on capture start
- **WHEN** audio capture begins (PTT key pressed or transcribe mode activated)
- **THEN** the mini waveform becomes visible and begins animating

#### Scenario: Mini waveform hides on capture stop
- **WHEN** audio capture stops (PTT key released and transcription completes, or transcribe mode deactivated)
- **THEN** the mini waveform is hidden

#### Scenario: Mini waveform opens visualization window
- **WHEN** the user double-clicks the mini waveform
- **THEN** the visualization window opens (or focuses if already open)

### Requirement: Visualization Window Drag Region
The visualization window background SHALL be draggable to allow window repositioning without a native title bar.

#### Scenario: User drags visualization window via background
- **WHEN** the user clicks and drags on any non-interactive background area of the visualization window
- **THEN** the window moves with the cursor to reposition on screen

#### Scenario: Interactive elements in visualization window remain functional
- **WHEN** the user clicks on a canvas in the visualization window
- **THEN** the element receives the click event normally without initiating window drag

### Requirement: Theme Color System
The application SHALL define all UI colors as CSS custom properties in a shared theme stylesheet. Both a dark and a light theme palette SHALL be provided. The dark theme SHALL use the existing color values. The light theme SHALL use a complementary color set that maintains equivalent visual hierarchy and sufficient contrast.

#### Scenario: Dark theme matches current appearance
- **WHEN** the dark theme is active
- **THEN** all UI colors match the existing hardcoded color values (gradients, text, borders, buttons, indicators)

#### Scenario: Light theme provides complementary colors
- **WHEN** the light theme is active
- **THEN** backgrounds use light neutral tones, text uses dark tones, and accent colors (blue, green, red, orange) are adjusted for contrast on light backgrounds

#### Scenario: Color tokens cover all UI elements
- **WHEN** any window renders in either theme
- **THEN** every visible color (background, text, border, button, indicator, scrollbar, input) is derived from a CSS custom property rather than a hardcoded value

#### Scenario: Canvas renderers use theme colors
- **WHEN** the waveform, spectrogram, or speech activity renderers draw on canvas
- **THEN** they read color values from CSS custom properties on the document root, adapting to the active theme

### Requirement: Theme Mode Selection
The application SHALL support three theme modes: "light", "dark", and "auto". The default mode SHALL be "auto".

#### Scenario: Auto mode follows OS preference
- **WHEN** the theme mode is set to "auto"
- **THEN** the application uses dark theme when the OS reports a dark color scheme preference, and light theme when the OS reports a light color scheme preference

#### Scenario: Auto mode responds to OS changes
- **WHEN** the theme mode is "auto" and the user changes the OS color scheme preference
- **THEN** the application switches to the corresponding theme without restart

#### Scenario: Light mode forces light theme
- **WHEN** the theme mode is set to "light"
- **THEN** the application uses the light theme regardless of the OS color scheme preference

#### Scenario: Dark mode forces dark theme
- **WHEN** the theme mode is set to "dark"
- **THEN** the application uses the dark theme regardless of the OS color scheme preference

### Requirement: Theme Persistence
The application SHALL persist the user's theme mode choice in the configuration file so it is restored on next launch.

#### Scenario: Theme mode saved to config
- **WHEN** the user changes the theme mode
- **THEN** the choice is saved to the configuration file

#### Scenario: Theme mode restored on launch
- **WHEN** the application launches
- **THEN** the theme mode from the configuration file is applied before the first paint

#### Scenario: Default theme mode for new installs
- **WHEN** no configuration file exists (first launch)
- **THEN** the theme mode defaults to "auto"

### Requirement: Theme Applied to All Windows
The application SHALL apply the active theme consistently to all windows: main, configuration, visualization, and about.

#### Scenario: Main window uses active theme
- **WHEN** the main window is visible
- **THEN** it renders with the active theme's color palette

#### Scenario: Config window uses active theme
- **WHEN** the configuration window is opened
- **THEN** it renders with the active theme's color palette

#### Scenario: Visualization window uses active theme
- **WHEN** the visualization window is opened
- **THEN** it renders with the active theme's color palette

#### Scenario: About window uses active theme
- **WHEN** the about window is opened
- **THEN** it renders with the active theme's color palette

#### Scenario: Theme change propagates to open windows
- **WHEN** the user changes the theme mode while multiple windows are open
- **THEN** all open windows update to the new theme without requiring a restart or window close/reopen

