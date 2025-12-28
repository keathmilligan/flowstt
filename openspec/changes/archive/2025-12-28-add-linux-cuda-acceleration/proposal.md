# Change: Add CUDA Acceleration for Linux Voice Transcription

## Why

Linux voice transcription currently uses CPU-only processing via the `whisper-rs` crate, which is significantly slower than GPU-accelerated inference. Adding opt-in CUDA support enables users with NVIDIA GPUs to achieve faster transcription speeds (typically 5-10x improvement for real-time transcription workloads).

## What Changes

- Add optional `cuda` feature flag to the Cargo configuration (Linux-only)
- Propagate the `cuda` feature to the `whisper-rs` dependency when enabled
- Update build documentation to explain CUDA build requirements and usage
- **No breaking changes**: CUDA is opt-in; default builds remain CPU-only

## Impact

- Affected specs: `speech-transcription`
- Affected code: `src-tauri/Cargo.toml`, build documentation
- Build requirements: When CUDA feature is enabled, requires NVIDIA CUDA Toolkit (nvcc, cuBLAS) installed on the build system
- Runtime requirements: When built with CUDA, requires NVIDIA GPU with compatible drivers
