## ADDED Requirements

### Requirement: Global Hotkey Backend Interface
The system SHALL provide a platform-agnostic interface for global hotkey capture through a `HotkeyBackend` trait. Platform-specific implementations SHALL implement this trait, enabling push-to-talk functionality across supported platforms.

#### Scenario: Backend trait defines hotkey operations
- **WHEN** the application requires global hotkey functionality
- **THEN** it uses the `HotkeyBackend` trait methods: `start()`, `stop()`, and `try_recv()`

#### Scenario: Backend selected at compile time
- **WHEN** the application is compiled for a specific platform
- **THEN** the appropriate platform backend is selected via conditional compilation

#### Scenario: Backend delivers press and release events
- **WHEN** the configured hotkey is pressed or released
- **THEN** the backend delivers a `HotkeyEvent::Pressed` or `HotkeyEvent::Released` event

### Requirement: macOS Hotkey Backend (CGEventTap)
The system SHALL provide a fully functional hotkey backend for macOS using the CGEventTap API, supporting global key monitoring even when the application is not focused.

#### Scenario: macOS backend initializes CGEventTap
- **WHEN** the hotkey backend starts on macOS
- **THEN** a CGEventTap is created in passive listening mode (kCGEventTapOptionListenOnly)

#### Scenario: macOS backend detects key events
- **WHEN** the configured hotkey is pressed or released anywhere in the system
- **THEN** the backend receives and processes the key event

#### Scenario: macOS backend runs on separate thread
- **WHEN** the hotkey backend is active
- **THEN** event monitoring runs on a dedicated thread to avoid blocking audio processing

#### Scenario: macOS backend stops cleanly
- **WHEN** the hotkey backend stop() is called
- **THEN** the CGEventTap is disabled and the run loop exits

#### Scenario: macOS backend filters to configured key
- **WHEN** key events are received
- **THEN** only events matching the configured hotkey generate HotkeyEvents

### Requirement: macOS Accessibility Permission
The system SHALL require Accessibility permission for global hotkey capture on macOS and provide clear feedback when permission is missing.

#### Scenario: Permission check on startup
- **WHEN** the hotkey backend attempts to start on macOS
- **THEN** the system checks for Accessibility permission before creating the CGEventTap

#### Scenario: Permission denied error
- **WHEN** Accessibility permission is not granted
- **THEN** the backend returns an error indicating Accessibility permission is required

#### Scenario: Permission granted allows monitoring
- **WHEN** Accessibility permission has been granted
- **THEN** global key monitoring proceeds normally

#### Scenario: Permission prompt guidance
- **WHEN** the system detects missing Accessibility permission
- **THEN** an error message includes instructions to enable it in System Preferences > Security & Privacy > Privacy > Accessibility

### Requirement: Windows Hotkey Backend (Stub)
The system SHALL provide a stub hotkey backend for Windows that compiles cleanly but returns appropriate errors indicating the feature is not yet implemented.

#### Scenario: Windows backend compiles
- **WHEN** the application is compiled on Windows
- **THEN** compilation succeeds using the stub backend

#### Scenario: Windows backend returns not-implemented error
- **WHEN** the hotkey backend start() is called on Windows
- **THEN** an error is returned indicating push-to-talk is not yet available on Windows

#### Scenario: Windows backend try_recv returns None
- **WHEN** try_recv() is called on the Windows stub backend
- **THEN** None is returned (no events)

### Requirement: Linux Hotkey Backend (Stub)
The system SHALL provide a stub hotkey backend for Linux that compiles cleanly but returns appropriate errors indicating the feature is not yet implemented.

#### Scenario: Linux backend compiles
- **WHEN** the application is compiled on Linux
- **THEN** compilation succeeds using the stub backend

#### Scenario: Linux backend returns not-implemented error
- **WHEN** the hotkey backend start() is called on Linux
- **THEN** an error is returned indicating push-to-talk is not yet available on Linux

#### Scenario: Linux backend try_recv returns None
- **WHEN** try_recv() is called on the Linux stub backend
- **THEN** None is returned (no events)

### Requirement: Hotkey Configuration
The system SHALL allow configuration of the push-to-talk hotkey, with a sensible default for each platform.

#### Scenario: Default hotkey on macOS
- **WHEN** no custom hotkey is configured on macOS
- **THEN** the Right Option (Alt) key is used as the PTT hotkey

#### Scenario: Custom hotkey configuration
- **WHEN** the user configures a custom PTT hotkey
- **THEN** the backend monitors for the specified key instead of the default

#### Scenario: Configuration persists across sessions
- **WHEN** the user configures a custom hotkey
- **THEN** the configuration is saved and restored on next application launch

#### Scenario: Invalid key rejected
- **WHEN** the user attempts to configure an invalid or restricted key
- **THEN** the configuration is rejected with an appropriate error message

### Requirement: Hotkey IPC Requests
The system SHALL support IPC requests for configuring and controlling the push-to-talk hotkey system.

#### Scenario: Set transcription mode request
- **WHEN** a client sends a SetTranscriptionMode request
- **THEN** the service updates the active transcription mode and responds with success

#### Scenario: Set PTT key request
- **WHEN** a client sends a SetPushToTalkKey request
- **THEN** the service updates the hotkey configuration and restarts monitoring with the new key

#### Scenario: Get transcription mode request
- **WHEN** a client sends a GetTranscriptionMode request
- **THEN** the service responds with the current transcription mode and hotkey configuration

### Requirement: Hotkey Event Key Codes
The system SHALL use platform-independent key code representation for configuring the push-to-talk hotkey.

#### Scenario: Key code enumeration
- **WHEN** a key is configured as the PTT hotkey
- **THEN** it is represented using a `KeyCode` enum that maps to platform-specific codes

#### Scenario: Common keys supported
- **WHEN** the user selects a PTT key
- **THEN** common choices are available including: RightAlt, LeftAlt, RightControl, LeftControl, RightShift, LeftShift, CapsLock, and function keys F13-F24

#### Scenario: Key code serialization
- **WHEN** key configuration is saved or transmitted via IPC
- **THEN** key codes serialize to human-readable names (e.g., "right_alt", "f13")
