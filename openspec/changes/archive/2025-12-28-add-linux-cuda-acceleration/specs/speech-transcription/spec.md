## ADDED Requirements

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
