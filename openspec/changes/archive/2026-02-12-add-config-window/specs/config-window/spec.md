## ADDED Requirements
### Requirement: Configuration Window
The system SHALL provide a configuration window for adjusting audio and input settings. The window is accessible from the system tray context menu.

#### Scenario: Config window opens from tray
- **WHEN** the user clicks "Settings" in the tray context menu
- **THEN** the configuration window opens centered on screen

#### Scenario: Config window appearance
- **WHEN** the configuration window is visible
- **THEN** it has the same dark theme as other application windows
- **AND** it has rounded corners with a subtle border
- **AND** it has a custom close button (no native title bar)
- **AND** it does not appear in the Windows taskbar

#### Scenario: Config window is draggable
- **WHEN** the user clicks and drags on any non-interactive background area of the configuration window
- **THEN** the window moves with the cursor to reposition on screen

#### Scenario: Config window close
- **GIVEN** the configuration window is visible
- **WHEN** the user clicks the close button
- **THEN** the configuration window closes
- **AND** the main application continues running

### Requirement: Input Device Configuration
The configuration window SHALL display a dropdown for selecting the primary audio input device, populated with the same device list as the main window.

#### Scenario: Input device dropdown populated on open
- **WHEN** the configuration window opens
- **THEN** the primary input device dropdown is populated with all available audio devices
- **AND** the currently selected device is pre-selected in the dropdown

#### Scenario: Input device changed in config window
- **WHEN** the user selects a different primary input device in the configuration window
- **THEN** the change takes effect immediately without requiring a save action
- **AND** the audio capture switches to the newly selected device

#### Scenario: No devices available
- **WHEN** no audio devices are detected
- **THEN** the dropdown displays a "None" option and no device is selected

### Requirement: Reference Input Configuration
The configuration window SHALL display a dropdown for selecting the reference/system audio input device, populated with the same device list as the main window.

#### Scenario: Reference input dropdown populated on open
- **WHEN** the configuration window opens
- **THEN** the reference input dropdown is populated with all available audio devices
- **AND** the currently selected reference device is pre-selected (or "None" if no reference is set)

#### Scenario: Reference input changed in config window
- **WHEN** the user selects a different reference input device in the configuration window
- **THEN** the change takes effect immediately without requiring a save action

#### Scenario: Reference input set to None
- **WHEN** the user selects "None" for the reference input
- **THEN** the system audio reference source is cleared

### Requirement: PTT Key Configuration
The configuration window SHALL display a dropdown for selecting the push-to-talk hotkey, using the same key options as the main window.

#### Scenario: PTT key dropdown populated on open
- **WHEN** the configuration window opens
- **THEN** the PTT key dropdown is populated with all available key options
- **AND** the currently configured PTT key is pre-selected

#### Scenario: PTT key changed in config window
- **WHEN** the user selects a different PTT key in the configuration window
- **THEN** the change takes effect immediately without requiring a save action
- **AND** the hotkey backend is reconfigured with the new key
