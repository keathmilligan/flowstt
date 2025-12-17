# audio-recording Specification

## Purpose
TBD - created by archiving change add-whisper-stt-scaffolding. Update Purpose after archive.
## Requirements
### Requirement: Audio Device Enumeration
The system SHALL enumerate all available audio input devices and system audio sources, presenting them for user selection based on the selected source type.

#### Scenario: Devices listed on load
- **WHEN** the application starts
- **THEN** a dropdown displays all available audio input devices by name for the default source type (Input)

#### Scenario: No devices available
- **WHEN** no audio input devices are detected for the selected source type
- **THEN** the UI displays a message indicating no devices found and disables recording

#### Scenario: Source type change
- **WHEN** the user changes the source type
- **THEN** the device dropdown is repopulated with devices appropriate for the new source type

### Requirement: Audio Recording Control
The system SHALL allow the user to start and stop audio recording from the selected input device.

#### Scenario: Start recording
- **WHEN** user clicks the record button with a device selected
- **THEN** audio capture begins from the selected device and the button indicates recording state

#### Scenario: Stop recording
- **WHEN** user clicks the record button while recording
- **THEN** audio capture stops and the recorded audio is prepared for transcription

### Requirement: Audio Format Conversion
The system SHALL convert recorded audio to 16kHz mono format for Whisper compatibility.

#### Scenario: High sample rate input
- **WHEN** the input device provides audio at a sample rate other than 16kHz
- **THEN** the audio is resampled to 16kHz before transcription

#### Scenario: Stereo input
- **WHEN** the input device provides stereo audio
- **THEN** the audio is converted to mono before transcription

### Requirement: Audio Source Type Selection
The system SHALL allow users to select the audio source type: input device (microphone), system audio (desktop output), or mixed (both combined).

#### Scenario: Input source type selected
- **WHEN** the user selects "Input" as the source type
- **THEN** the device dropdown displays available microphone and input devices

#### Scenario: System source type selected
- **WHEN** the user selects "System" as the source type
- **THEN** the device dropdown displays available system audio sources (monitor devices)

#### Scenario: Mixed source type selected
- **WHEN** the user selects "Mixed" as the source type
- **THEN** the device dropdown displays available input devices and the system captures from both the selected input and the default system audio output

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

### Requirement: System Audio Recording
The system SHALL capture audio from system audio sources (monitor devices) for monitoring and recording, using the same processing pipeline as input devices.

#### Scenario: Start system audio monitoring
- **WHEN** user starts monitoring with a system audio source selected
- **THEN** audio capture begins from the monitor device and visualization displays the system audio

#### Scenario: Start system audio recording
- **WHEN** user starts recording with a system audio source selected
- **THEN** audio is captured from the monitor device and prepared for transcription

#### Scenario: System audio format conversion
- **WHEN** the system audio source provides audio at a sample rate other than 16kHz
- **THEN** the audio is resampled to 16kHz before transcription

### Requirement: Mixed Audio Capture
The system SHALL support capturing audio from both an input device and system audio simultaneously, combining them into a single stream. When both sources are active, acoustic echo cancellation SHALL be applied to the microphone input to remove system audio that is picked up acoustically before mixing.

#### Scenario: Mixed mode start
- **WHEN** user starts monitoring or recording in mixed mode
- **THEN** audio is captured from both the selected input device and system audio output simultaneously

#### Scenario: Mixed mode audio combination
- **WHEN** capturing in mixed mode
- **THEN** input and system audio samples are mixed with equal gain (0.5 each) to prevent clipping

#### Scenario: Mixed mode visualization
- **WHEN** monitoring in mixed mode
- **THEN** the waveform and spectrogram display the combined audio from both sources

#### Scenario: Mixed mode transcription
- **WHEN** recording completes in mixed mode
- **THEN** the combined audio is transcribed, capturing speech from both microphone and system audio

#### Scenario: Echo cancellation applied in mixed mode
- **WHEN** capturing in mixed mode with both microphone and system audio active
- **THEN** acoustic echo cancellation is applied to the microphone signal using system audio as the reference before mixing, removing speaker feedback from the microphone input

#### Scenario: Echo cancellation improves transcription
- **WHEN** system audio is playing while user speaks into microphone in mixed mode
- **THEN** the user's speech is clearly captured without duplication of the system audio content

### Requirement: Echo Cancellation Recording Mode
The system SHALL provide a recording mode selection that determines how audio from multiple sources is combined. The available modes are "Mixed" (combine both streams) and "Echo Cancel" (output only the primary stream with echo removed).

#### Scenario: Mixed mode selected (default)
- **WHEN** the user selects "Mixed" recording mode
- **THEN** audio capture combines the primary and secondary sources as per existing Mixed Audio Capture behavior

#### Scenario: Echo Cancel mode selected
- **WHEN** the user selects "Echo Cancel" recording mode with both primary and secondary sources active
- **THEN** the system uses the secondary source as an AEC reference signal and outputs only the echo-cancelled primary source

#### Scenario: Echo Cancel mode with single source
- **WHEN** the user attempts to select "Echo Cancel" mode with only one source active
- **THEN** the UI prevents selection or indicates that two sources are required for this mode

#### Scenario: Echo Cancel mode produces voice-only output
- **WHEN** recording in Echo Cancel mode while system audio is playing
- **THEN** the recorded output contains only the user's voice with system audio removed

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

