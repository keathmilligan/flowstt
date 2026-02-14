## ADDED Requirements

### Requirement: Theme Mode Configuration
The configuration window SHALL provide a control for selecting the application theme mode. The available options SHALL be "Auto" (follow system), "Light", and "Dark".

#### Scenario: Theme selector displayed in config window
- **WHEN** the configuration window opens
- **THEN** a theme mode selector is displayed with three options: "Auto", "Light", and "Dark"
- **AND** the currently active theme mode is pre-selected

#### Scenario: User selects light mode
- **WHEN** the user selects "Light" from the theme selector
- **THEN** the application switches to light theme immediately
- **AND** the change is persisted to the configuration file

#### Scenario: User selects dark mode
- **WHEN** the user selects "Dark" from the theme selector
- **THEN** the application switches to dark theme immediately
- **AND** the change is persisted to the configuration file

#### Scenario: User selects auto mode
- **WHEN** the user selects "Auto" from the theme selector
- **THEN** the application applies the theme matching the current OS color scheme preference
- **AND** the change is persisted to the configuration file

#### Scenario: Theme change takes effect without save action
- **WHEN** the user changes the theme selector value
- **THEN** the change takes effect immediately without requiring a separate save or apply action

## MODIFIED Requirements

### Requirement: Configuration Window
The system SHALL provide a configuration window for adjusting audio, input, and appearance settings. The window is accessible from the system tray context menu. The window SHALL be sized to accommodate the hotkey management interface and theme selector.

#### Scenario: Config window opens from tray
- **WHEN** the user clicks "Settings" in the tray context menu
- **THEN** the configuration window opens centered on screen

#### Scenario: Config window appearance
- **WHEN** the configuration window is visible
- **THEN** it uses the active theme's color palette
- **AND** it has rounded corners with a subtle border
- **AND** it has a custom close button (no native title bar)
- **AND** it does not appear in the Windows taskbar

#### Scenario: Config window enlarged dimensions
- **WHEN** the configuration window is created
- **THEN** its dimensions are approximately 480x460 logical pixels to accommodate the hotkey binding list, theme selector, and recorder widget

#### Scenario: Config window is draggable
- **WHEN** the user clicks and drags on any non-interactive background area of the configuration window
- **THEN** the window moves with the cursor to reposition on screen

#### Scenario: Config window close
- **GIVEN** the configuration window is visible
- **WHEN** the user clicks the close button
- **THEN** the configuration window closes
- **AND** the main application continues running
