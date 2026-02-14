## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Gradient Background
The application SHALL display a gradient background across the entire window, using theme-appropriate colors. In dark mode, the gradient uses dark gray tones. In light mode, the gradient uses light neutral tones.

#### Scenario: Background renders on launch
- **WHEN** the application window opens
- **THEN** the background displays a smooth gradient appropriate to the active theme

#### Scenario: Background adapts to theme change
- **WHEN** the user switches the theme mode
- **THEN** the gradient background transitions to the new theme's color palette
