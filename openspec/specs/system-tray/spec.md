# system-tray Specification

## Purpose
TBD - created by archiving change add-system-tray. Update Purpose after archive.
## Requirements
### Requirement: Tray Icon Display

The application SHALL display a tray icon in the Windows notification area when running.

#### Scenario: Application startup shows tray icon
- **WHEN** the FlowSTT application launches
- **THEN** a FlowSTT icon appears in the Windows system tray
- **AND** the icon has a tooltip showing "FlowSTT"

#### Scenario: Tray icon persists when window is hidden
- **GIVEN** the main window is visible
- **WHEN** the user closes the main window
- **THEN** the window is hidden (not destroyed)
- **AND** the tray icon remains visible in the system tray

### Requirement: Context Menu

The tray icon SHALL display a context menu when right-clicked, containing Show, Settings, About, and Exit items.

#### Scenario: Right-click shows context menu
- **WHEN** the user right-clicks the tray icon
- **THEN** a context menu appears immediately
- **AND** the menu contains "Show", "Settings", "About", and "Exit" items

#### Scenario: Show menu item
- **GIVEN** the context menu is displayed
- **WHEN** the user clicks "Show"
- **THEN** the main window becomes visible
- **AND** the main window is brought to the foreground

#### Scenario: Settings menu item
- **GIVEN** the context menu is displayed
- **WHEN** the user clicks "Settings"
- **THEN** the configuration window opens
- **AND** the configuration window is brought to the foreground

#### Scenario: Settings menu item with config window already open
- **GIVEN** the context menu is displayed
- **AND** the configuration window is already open
- **WHEN** the user clicks "Settings"
- **THEN** the existing configuration window is brought to the foreground

#### Scenario: About menu item
- **GIVEN** the context menu is displayed
- **WHEN** the user clicks "About"
- **THEN** an About window appears
- **AND** the About window displays application information

#### Scenario: Exit menu item
- **GIVEN** the context menu is displayed
- **WHEN** the user clicks "Exit"
- **THEN** the application exits completely
- **AND** the tray icon is removed

### Requirement: Double-Click Activation

Double-clicking the tray icon SHALL show the main window and bring it to the foreground.

#### Scenario: Double-click shows window
- **GIVEN** the main window is hidden
- **WHEN** the user double-clicks the tray icon
- **THEN** the main window becomes visible
- **AND** the main window is brought to the foreground

### Requirement: Hide to Tray

Closing the main window SHALL hide it to the tray instead of exiting the application.

#### Scenario: Close button hides to tray
- **GIVEN** the main window is visible
- **WHEN** the user clicks the window close button
- **THEN** the main window is hidden
- **AND** the application continues running
- **AND** the tray icon remains visible

### Requirement: About Window

The About window SHALL display application information with styling consistent with the main window.

#### Scenario: About window appearance
- **WHEN** the About window opens
- **THEN** it displays the FlowSTT logo
- **AND** it displays the application name "FlowSTT"
- **AND** it displays the current version number
- **AND** it displays a brief description
- **AND** it displays links to website and GitHub

#### Scenario: About window styling
- **WHEN** the About window is visible
- **THEN** it has the same dark theme as the main window
- **AND** it has rounded corners
- **AND** it has a custom close button (no native title bar)
- **AND** it does not appear in the Windows taskbar

#### Scenario: About window close
- **GIVEN** the About window is visible
- **WHEN** the user clicks the close button
- **THEN** the About window closes
- **AND** the main application continues running

### Requirement: Window Recreation

The application SHALL handle window destruction gracefully when the main window is destroyed after being hidden.

#### Scenario: Window recreation after destruction
- **GIVEN** the main window was hidden and subsequently destroyed by Windows
- **WHEN** the user requests to show the window via tray menu or double-click
- **THEN** a new main window is created with the correct configuration
- **AND** the window is brought to the foreground

