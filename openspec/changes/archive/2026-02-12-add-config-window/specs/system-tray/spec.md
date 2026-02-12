## MODIFIED Requirements
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
