## ADDED Requirements

### Requirement: Platform-Agnostic Audio Backend Interface
The system SHALL provide a platform-agnostic interface for audio capture operations through an `AudioBackend` trait. Platform-specific implementations SHALL implement this trait, enabling the application to function identically across supported platforms.

#### Scenario: Backend trait defines capture operations
- **WHEN** the application requires audio capture functionality
- **THEN** it uses the `AudioBackend` trait methods: `list_input_devices()`, `list_system_devices()`, `start_capture_sources()`, `stop_capture()`, and `try_recv()`

#### Scenario: Backend selected at compile time
- **WHEN** the application is compiled for a specific platform
- **THEN** the appropriate platform backend is selected via conditional compilation

#### Scenario: Backend provides consistent sample format
- **WHEN** any platform backend delivers audio samples
- **THEN** samples are provided as stereo f32 interleaved format with the backend's native sample rate

### Requirement: Linux Audio Backend (PipeWire)
The system SHALL provide a fully functional audio backend for Linux using PipeWire, supporting all audio capture features including input device capture, system audio capture, mixing, and echo cancellation.

#### Scenario: Linux backend initializes PipeWire
- **WHEN** the application starts on Linux
- **THEN** the PipeWire-based backend is initialized and device enumeration begins

#### Scenario: Linux backend captures input audio
- **WHEN** the user selects an input device and starts capture on Linux
- **THEN** audio is captured from the selected PipeWire input source

#### Scenario: Linux backend captures system audio
- **WHEN** the user selects a system audio source on Linux
- **THEN** audio is captured from the PipeWire sink monitor

### Requirement: Windows Audio Backend (Stub)
The system SHALL provide a stub audio backend for Windows that compiles successfully but returns "not implemented" errors for all operations. This establishes the infrastructure for future Windows audio support.

#### Scenario: Windows backend compiles
- **WHEN** the application is compiled on Windows
- **THEN** compilation succeeds using the stub backend

#### Scenario: Windows backend returns not implemented
- **WHEN** the user attempts any audio operation on Windows
- **THEN** the system returns an error indicating audio is not yet implemented for Windows

#### Scenario: Windows device enumeration returns empty
- **WHEN** device enumeration is requested on Windows
- **THEN** empty device lists are returned

### Requirement: macOS Audio Backend (Stub)
The system SHALL provide a stub audio backend for macOS that compiles successfully but returns "not implemented" errors for all operations. This establishes the infrastructure for future macOS audio support.

#### Scenario: macOS backend compiles
- **WHEN** the application is compiled on macOS
- **THEN** compilation succeeds using the stub backend

#### Scenario: macOS backend returns not implemented
- **WHEN** the user attempts any audio operation on macOS
- **THEN** the system returns an error indicating audio is not yet implemented for macOS

#### Scenario: macOS device enumeration returns empty
- **WHEN** device enumeration is requested on macOS
- **THEN** empty device lists are returned

### Requirement: Platform-Independent Device Representation
The system SHALL represent audio devices using a platform-independent structure that can be serialized for frontend communication. Device IDs SHALL be strings to accommodate different platform ID formats.

#### Scenario: Device has string ID
- **WHEN** a device is enumerated on any platform
- **THEN** the device ID is represented as a string

#### Scenario: Device includes source type
- **WHEN** a device is enumerated
- **THEN** the device indicates whether it is an Input or System audio source

#### Scenario: Device has human-readable name
- **WHEN** a device is enumerated
- **THEN** the device includes a user-friendly display name

## MODIFIED Requirements

### Requirement: System Audio Device Enumeration
The system SHALL enumerate available system audio sources (monitor/loopback devices) using the platform-appropriate audio backend. On Linux, this uses PipeWire or PulseAudio monitor sources. On Windows and macOS, the stub backend returns an empty list until full support is implemented.

#### Scenario: Monitor sources available (Linux)
- **WHEN** the system has active audio output devices on Linux with PipeWire or PulseAudio
- **THEN** corresponding monitor sources are listed as system audio devices

#### Scenario: No monitor sources available
- **WHEN** no system audio output devices are active or the platform backend does not support system audio
- **THEN** the system audio device list is empty and the UI indicates no system audio sources found

#### Scenario: Monitor source naming
- **WHEN** enumerating system audio devices
- **THEN** devices are displayed with user-friendly names derived from the output device name
