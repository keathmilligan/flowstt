# speech-transcription Specification

## Purpose
TBD - created by archiving change add-whisper-stt-scaffolding. Update Purpose after archive.
## Requirements
### Requirement: Local Whisper Transcription
The system SHALL transcribe recorded audio to text using a local Whisper model. On Windows and macOS, transcription uses the whisper.cpp shared library loaded via FFI. On Linux, transcription uses the whisper-rs crate. In transcribe mode, segments are processed asynchronously from a queue.

#### Scenario: Successful transcription
- **WHEN** recording stops and audio data is available
- **THEN** the audio is transcribed and the resulting text is displayed in the UI

#### Scenario: Transcription in progress
- **WHEN** transcription is processing
- **THEN** the UI displays a loading indicator

#### Scenario: Windows/macOS library loading
- **WHEN** transcription is requested on Windows or macOS
- **THEN** the whisper.cpp shared library (whisper.dll or libwhisper.dylib) is loaded from the application bundle

#### Scenario: Linux transcription
- **WHEN** transcription is requested on Linux
- **THEN** transcription is performed using the whisper-rs crate

#### Scenario: Queue-based transcription in transcribe mode
- **WHEN** transcribe mode is active and a speech segment is queued
- **THEN** the transcription worker processes the segment from the queue and emits the result

### Requirement: Model Loading
The system SHALL load the Whisper model from a known filesystem location.

#### Scenario: Model found
- **WHEN** the model file exists at the expected path
- **THEN** the model loads successfully and transcription is available

#### Scenario: Model not found
- **WHEN** the model file does not exist at the expected path
- **THEN** the system displays an error message with instructions for obtaining the model

### Requirement: Transcription Result Display
The system SHALL display transcription results in a dedicated text area.

#### Scenario: Display transcribed text
- **WHEN** transcription completes successfully
- **THEN** the transcribed text appears in the result area

#### Scenario: Empty transcription
- **WHEN** transcription completes but no speech was detected
- **THEN** the result area indicates no speech was detected

### Requirement: Whisper Library Bundling
The system SHALL bundle the whisper.cpp shared library with the application on Windows and macOS. The library SHALL be downloaded from the official whisper.cpp GitHub releases during the build process.

#### Scenario: Build downloads library
- **WHEN** the application is built on Windows or macOS
- **THEN** the build process downloads the appropriate whisper.cpp binary from GitHub releases if not already cached

#### Scenario: Library bundled with application
- **WHEN** the application is packaged for distribution
- **THEN** the whisper.dll (Windows) or libwhisper.dylib (macOS) is included in the application bundle

#### Scenario: Cached binary reused
- **WHEN** building and the whisper.cpp binary for the target version already exists in the build cache
- **THEN** the cached binary is used without re-downloading

#### Scenario: Download failure handling
- **WHEN** the build process cannot download the whisper.cpp binary (network error, GitHub unavailable)
- **THEN** the build fails with a clear error message indicating the download failure

### Requirement: Platform-Specific Binary Selection
The build system SHALL select the correct whisper.cpp binary based on the target platform and architecture.

#### Scenario: Windows x64 build
- **WHEN** building for Windows x64
- **THEN** the `whisper-bin-x64.zip` binary is downloaded and whisper.dll is extracted

#### Scenario: Windows x86 build
- **WHEN** building for Windows x86
- **THEN** the `whisper-bin-Win32.zip` binary is downloaded and whisper.dll is extracted

#### Scenario: macOS build
- **WHEN** building for macOS
- **THEN** the `whisper-v{version}-xcframework.zip` is downloaded and the correct architecture dylib is extracted

#### Scenario: Linux build
- **WHEN** building for Linux
- **THEN** no binary download occurs; the whisper-rs crate builds whisper.cpp from source

### Requirement: Transcription Queue
The system SHALL maintain a queue of audio segments awaiting transcription. The queue allows recording to continue while transcription processes previous segments asynchronously.

#### Scenario: Segment queued for transcription
- **WHEN** a speech segment is finalized in transcribe mode
- **THEN** the segment is added to the transcription queue

#### Scenario: Queue processes segments sequentially
- **WHEN** multiple segments are in the transcription queue
- **THEN** segments are processed in FIFO order (oldest first)

#### Scenario: Queue bounded to prevent memory growth
- **WHEN** the transcription queue reaches its maximum capacity (10 segments)
- **THEN** new segments wait until space is available (recording continues, queue blocks)

#### Scenario: Queue drains on transcribe mode stop
- **WHEN** the user disables transcribe mode
- **THEN** remaining queued segments continue to be transcribed until the queue is empty

### Requirement: Async Transcription Worker
The system SHALL run a dedicated worker thread that processes queued transcription segments independently of the recording and monitoring pipeline.

#### Scenario: Worker processes queue
- **WHEN** segments are present in the transcription queue
- **THEN** the worker retrieves and transcribes segments one at a time

#### Scenario: Worker emits results
- **WHEN** the worker completes transcription of a segment
- **THEN** a transcription-complete event is emitted with the transcribed text

#### Scenario: Worker handles empty segments
- **WHEN** a queued segment contains no detectable speech
- **THEN** the worker emits "(No speech detected)" as the transcription result

#### Scenario: Worker tolerates transcription lag
- **WHEN** speech segments are produced faster than transcription can process them
- **THEN** the worker continues processing at its own pace while segments accumulate in the queue

### Requirement: CUDA GPU Acceleration (Linux)
On Linux, the system SHALL support optional CUDA GPU acceleration for voice transcription when built with the `cuda` feature flag. When enabled, transcription uses NVIDIA GPU hardware for faster inference.

#### Scenario: CUDA-enabled build on Linux
- **WHEN** the application is built on Linux with `--features cuda`
- **THEN** the whisper-rs crate is compiled with CUDA support
- **AND** transcription uses the NVIDIA GPU when a compatible GPU and drivers are present

#### Scenario: Default CPU-only build on Linux
- **WHEN** the application is built on Linux without the `cuda` feature flag
- **THEN** transcription uses CPU-only processing (existing behavior)

#### Scenario: CUDA feature ignored on other platforms
- **WHEN** the `cuda` feature flag is specified on Windows or macOS builds
- **THEN** the feature has no effect (those platforms use prebuilt FFI binaries)

#### Scenario: CUDA build without GPU at runtime
- **WHEN** the application is built with CUDA support
- **AND** no compatible NVIDIA GPU is available at runtime
- **THEN** transcription falls back to CPU processing

