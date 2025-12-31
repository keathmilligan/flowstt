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
The system SHALL allow the user to control audio capture through the Transcribe toggle. When Transcribe is active, audio capture runs continuously and speech segments are automatically extracted based on speech detection. Manual recording control is no longer supported.

#### Scenario: Start transcribe mode
- **WHEN** user enables the Transcribe toggle with a device selected
- **THEN** continuous audio capture begins and speech-triggered segment extraction becomes active

#### Scenario: Stop transcribe mode
- **WHEN** user disables the Transcribe toggle
- **THEN** any in-progress speech segment is finalized, audio capture stops, and the ring buffer is released

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
The system SHALL enumerate available system audio sources (monitor/loopback devices) using the platform-appropriate audio backend. On Linux, this uses PipeWire or PulseAudio monitor sources. On Windows, this uses WASAPI loopback mode to enumerate render endpoints as capturable system audio sources. On macOS, the stub backend returns an empty list until full support is implemented.

#### Scenario: Monitor sources available (Linux)
- **WHEN** the system has active audio output devices on Linux with PipeWire or PulseAudio
- **THEN** corresponding monitor sources are listed as system audio devices

#### Scenario: Loopback sources available (Windows)
- **WHEN** the system has active audio render endpoints on Windows
- **THEN** corresponding loopback sources are listed as system audio devices with user-friendly names

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

### Requirement: macOS Audio Backend (Stub)
The system SHALL provide a fully functional audio backend for macOS using CoreAudio for input capture and ScreenCaptureKit for system audio capture. The backend supports input device enumeration, system audio enumeration, single-source capture, multi-source capture with mixing, and echo cancellation, achieving feature parity with Linux and Windows backends.

#### Scenario: macOS backend compiles
- **WHEN** the application is compiled on macOS
- **THEN** compilation succeeds using the CoreAudio and ScreenCaptureKit backend

#### Scenario: macOS input device enumeration
- **WHEN** input device enumeration is requested on macOS
- **THEN** available input devices (microphones) are returned with their names and IDs

#### Scenario: macOS single-source capture starts
- **WHEN** the user starts capture with a single input device selected on macOS
- **THEN** audio capture begins from the selected device and samples are delivered via the backend interface

#### Scenario: macOS single-source capture stops
- **WHEN** the user stops capture on macOS
- **THEN** audio capture stops and resources are released

#### Scenario: macOS backend provides consistent sample format
- **WHEN** the macOS backend delivers audio samples
- **THEN** samples are provided as stereo f32 interleaved format at 48kHz (resampled if device uses different rate)

#### Scenario: macOS format conversion
- **WHEN** the input device provides audio in a format other than f32 stereo at 48kHz
- **THEN** the audio is converted to f32 stereo at 48kHz before delivery

#### Scenario: macOS mono input handling
- **WHEN** the input device provides mono audio
- **THEN** the audio is converted to stereo by duplicating samples

#### Scenario: macOS system audio enumeration
- **WHEN** system audio device enumeration is requested on macOS 12.3+
- **THEN** available system audio outputs are returned via ScreenCaptureKit with user-friendly names

#### Scenario: macOS system audio enumeration on older macOS
- **WHEN** system audio device enumeration is requested on macOS versions prior to 12.3
- **THEN** an empty device list is returned (ScreenCaptureKit not available)

#### Scenario: macOS system audio capture starts
- **WHEN** the user starts capture with a system audio source selected on macOS
- **THEN** audio capture begins from the system audio output using ScreenCaptureKit

#### Scenario: macOS system audio requires permission
- **WHEN** the user attempts to capture system audio without Screen Recording permission
- **THEN** the system returns an error indicating Screen Recording permission is required

#### Scenario: macOS multi-source capture starts
- **WHEN** the user starts capture with both an input device and system audio source on macOS
- **THEN** audio capture begins from both sources simultaneously using separate capture threads

#### Scenario: macOS multi-source audio mixing
- **WHEN** capturing from both input and system sources on macOS
- **THEN** samples from both sources are mixed using frame-based processing (10ms frames at 48kHz)

#### Scenario: macOS echo cancellation applied
- **WHEN** capturing from both sources on macOS with echo cancellation enabled
- **THEN** the AEC3 algorithm is applied to the microphone signal using system audio as reference

#### Scenario: macOS recording mode Mixed
- **WHEN** capturing from both sources on macOS in Mixed recording mode
- **THEN** echo-cancelled microphone and system audio are combined with soft clipping to prevent distortion

#### Scenario: macOS recording mode EchoCancel
- **WHEN** capturing from both sources on macOS in EchoCancel recording mode
- **THEN** only the echo-cancelled microphone signal is output (no system audio in output)

#### Scenario: macOS excludes app audio from system capture
- **WHEN** capturing system audio on macOS
- **THEN** the application's own audio output is excluded from the captured audio to prevent feedback loops

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

### Requirement: Windows Audio Backend (Full)
The system SHALL provide a fully functional audio backend for Windows using WASAPI, supporting all audio capture features including input device capture, system audio capture (loopback), multi-source mixing, and echo cancellation. This achieves feature parity with the Linux PipeWire backend.

#### Scenario: Windows backend compiles
- **WHEN** the application is compiled on Windows
- **THEN** compilation succeeds using the WASAPI backend

#### Scenario: Windows input device enumeration
- **WHEN** device enumeration is requested on Windows
- **THEN** available input devices (microphones) are returned with their names and IDs

#### Scenario: Windows system audio enumeration
- **WHEN** system audio device enumeration is requested on Windows
- **THEN** available render endpoints are returned as loopback sources with their names and IDs

#### Scenario: Windows single-source capture starts
- **WHEN** the user starts capture with a single input device selected on Windows
- **THEN** audio capture begins from the selected device and samples are delivered via the backend interface

#### Scenario: Windows single-source capture stops
- **WHEN** the user stops capture on Windows
- **THEN** audio capture stops and resources are released

#### Scenario: Windows loopback capture starts
- **WHEN** the user starts capture with a system audio source selected on Windows
- **THEN** audio capture begins from the selected render endpoint using WASAPI loopback mode

#### Scenario: Windows multi-source capture starts
- **WHEN** the user starts capture with both an input device and system audio source on Windows
- **THEN** audio capture begins from both sources simultaneously using separate capture threads

#### Scenario: Windows multi-source audio mixing
- **WHEN** capturing from both input and system sources on Windows
- **THEN** samples from both sources are mixed using frame-based processing (10ms frames at 48kHz)

#### Scenario: Windows echo cancellation applied
- **WHEN** capturing from both sources on Windows with echo cancellation enabled
- **THEN** the AEC3 algorithm is applied to the microphone signal using system audio as reference

#### Scenario: Windows recording mode Mixed
- **WHEN** capturing from both sources on Windows in Mixed recording mode
- **THEN** echo-cancelled microphone and system audio are combined with soft clipping to prevent distortion

#### Scenario: Windows recording mode EchoCancel
- **WHEN** capturing from both sources on Windows in EchoCancel recording mode
- **THEN** only the echo-cancelled microphone signal is output (no system audio in output)

#### Scenario: Windows backend provides consistent sample format
- **WHEN** the Windows backend delivers audio samples
- **THEN** samples are provided as stereo f32 interleaved format at 48kHz (resampled if device uses different rate)

### Requirement: Automatic Transcription Mode
The system SHALL provide an automatic transcription mode where audio is captured continuously and speech segments are extracted for transcription based on speech detection events. When enabled, the system monitors for speech activity and extracts each speech segment for transcription without manual intervention. Segments SHALL be submitted for transcription after a maximum duration of 1 second, breaking at the next detected word boundary.

#### Scenario: Transcribe mode enabled
- **WHEN** the user enables the Transcribe toggle
- **THEN** the system begins continuous audio capture, monitoring for speech activity

#### Scenario: Continuous capture while transcribe active
- **WHEN** transcribe mode is active
- **THEN** audio samples are continuously written to a ring buffer regardless of speech state

#### Scenario: Speech triggers segment marking
- **WHEN** transcribe mode is active and the speech detector emits a speech-started event
- **THEN** the system marks the segment start position (including lookback samples) without interrupting capture

#### Scenario: Speech end triggers segment extraction
- **WHEN** transcribe mode is active and the speech detector emits a speech-ended event
- **THEN** the system extracts (copies) the segment from the ring buffer, saves it to a WAV file, and queues it for transcription

#### Scenario: Capture continues after segment extraction
- **WHEN** a speech segment is extracted from the ring buffer
- **THEN** audio capture continues uninterrupted, ready to capture the next segment

#### Scenario: Transcribe mode disabled
- **WHEN** the user disables the Transcribe toggle
- **THEN** the system stops audio capture and any in-progress segment is finalized and queued

#### Scenario: Segment submitted at word break after duration threshold
- **WHEN** transcribe mode is active and a speech segment exceeds 1 second duration
- **THEN** the system waits for the next word break event and extracts the segment up to that word boundary

#### Scenario: Segment submitted after grace period if no word break
- **WHEN** a speech segment exceeds 1 second duration and no word break is detected within 500ms
- **THEN** the system extracts the segment at the current position regardless of word boundary

#### Scenario: Speech state continues after timed segment
- **WHEN** a segment is extracted due to duration threshold (not speech end)
- **THEN** the system remains in speech state and begins a new segment from the word break position

### Requirement: Speech Segment Ring Buffer
The system SHALL maintain a ring buffer for continuous audio capture during transcribe mode. The ring buffer allows segment extraction without interrupting the audio stream, ensuring no samples are dropped between speech segments.

#### Scenario: Ring buffer sized for long utterances
- **WHEN** transcribe mode is initialized
- **THEN** the ring buffer is sized to hold at least 30 seconds of audio at the capture sample rate

#### Scenario: Samples continuously written
- **WHEN** audio samples arrive from the capture backend
- **THEN** samples are written to the ring buffer at the current write position, overwriting old samples when the buffer wraps

#### Scenario: Segment extraction copies samples
- **WHEN** a speech segment is extracted
- **THEN** samples are copied from the ring buffer into a new owned buffer for transcription, leaving the ring buffer intact

#### Scenario: Lookback samples included via ring buffer
- **WHEN** a speech-started event provides a lookback offset
- **THEN** the segment start position is set to include lookback samples already present in the ring buffer

#### Scenario: Buffer overflow triggers segment split
- **WHEN** a speech segment approaches ring buffer capacity (90% full) while speech continues
- **THEN** the current segment is extracted and queued, and a new segment begins at the current position without dropping any audio samples

#### Scenario: Split segment continues speech state
- **WHEN** a segment is split due to buffer overflow
- **THEN** the system remains in speech state, ready to extract the continuation when speech ends or another overflow occurs

### Requirement: Speech Segment Recording
The system SHALL capture speech segments as independent audio recordings, with each segment starting from the lookback-determined speech start point. Segment boundaries are determined by speech detection events within the continuous audio stream.

#### Scenario: Segment includes lookback audio
- **WHEN** a speech-started event triggers segment marking
- **THEN** the segment start position includes samples from the lookback period (capturing the true start of speech)

#### Scenario: Segment ends at speech end
- **WHEN** a speech-ended event is received
- **THEN** the segment ends at the current ring buffer position and is extracted for transcription

#### Scenario: Segment saved to WAV file
- **WHEN** a speech segment is extracted
- **THEN** the audio is saved to a WAV file in the configured recordings directory with a timestamped filename

#### Scenario: Long speech produces multiple segments
- **WHEN** continuous speech exceeds the ring buffer capacity
- **THEN** the speech is split into multiple segments, each saved as a separate WAV file and queued for transcription independently

### Requirement: ScreenCaptureKit Permission Handling
The system SHALL handle ScreenCaptureKit permission requirements on macOS gracefully, providing clear feedback to users when permission is needed or denied.

#### Scenario: Permission check before system audio capture
- **WHEN** the user attempts to capture system audio on macOS
- **THEN** the system verifies Screen Recording permission status before starting capture

#### Scenario: Permission denied feedback
- **WHEN** Screen Recording permission is denied or not granted
- **THEN** the system returns a clear error message indicating permission is required and where to enable it

#### Scenario: Permission granted allows capture
- **WHEN** Screen Recording permission has been granted
- **THEN** system audio capture proceeds normally via ScreenCaptureKit

#### Scenario: No permission prompt on input-only capture
- **WHEN** the user captures only from an input device (microphone)
- **THEN** no Screen Recording permission is requested (only Microphone permission applies)

### Requirement: macOS Minimum Version for System Audio
The system SHALL require macOS 12.3 or later for system audio capture functionality, as ScreenCaptureKit is not available on earlier versions.

#### Scenario: macOS version check for system audio
- **WHEN** the application starts on macOS
- **THEN** system audio features are enabled only if running on macOS 12.3 or later

#### Scenario: Older macOS shows no system devices
- **WHEN** running on macOS versions prior to 12.3
- **THEN** the system audio device list is empty and Mixed source type shows only input device

#### Scenario: Feature availability indication
- **WHEN** system audio features are unavailable due to macOS version
- **THEN** the UI indicates that system audio requires macOS 12.3 or later

### Requirement: Timed Segment Submission
The system SHALL limit speech segment duration to a maximum of 1 second during automatic transcription mode. When the duration threshold is reached, the segment is submitted at the next detected word break to produce incremental transcription results.

#### Scenario: Short utterance submitted on speech end
- **WHEN** speech lasts less than 1 second before ending
- **THEN** the segment is submitted when speech-ended is detected (existing behavior)

#### Scenario: Long utterance submitted at word break
- **WHEN** speech continues beyond 1 second
- **THEN** the segment is submitted at the next word break after the 1-second threshold

#### Scenario: Segment duration tracked from speech start
- **WHEN** a speech-started event occurs
- **THEN** the segment duration counter begins at zero (not counting lookback samples)

#### Scenario: Duration counter resets after timed submission
- **WHEN** a segment is submitted due to duration threshold
- **THEN** the duration counter resets to zero for the new segment

#### Scenario: Multiple timed segments from single utterance
- **WHEN** continuous speech lasts several seconds
- **THEN** multiple segments are extracted and queued at approximately 1-second intervals aligned to word breaks

