# Change: Refactor Backend for Multi-Platform Audio Support

## Why
The application currently only supports Linux via PipeWire for audio capture. To reach a broader user base, the backend needs to be reorganized with a platform abstraction layer that supports Windows, macOS, and Linux. This change establishes the architecture for cross-platform audio while maintaining full Linux functionality and adding stub implementations for Windows and macOS.

## What Changes
- **BREAKING**: Reorganize backend module structure with platform-specific directories
- Introduce `AudioBackend` trait defining the platform-agnostic audio capture interface
- Move PipeWire implementation to `platform/linux/` as the reference implementation
- Add stub implementations for `platform/windows/` and `platform/macos/` that return "not implemented" errors
- Update `lib.rs` to use conditional compilation (`#[cfg(target_os = "...")]`) for platform selection
- Update `Cargo.toml` with platform-specific dependencies
- Frontend remains unchanged (already platform-independent)

## Impact
- Affected specs: `audio-recording` (new platform abstraction requirement)
- Affected code:
  - `src-tauri/src/lib.rs` - platform selection logic
  - `src-tauri/src/pipewire_audio.rs` - moved to `platform/linux/pipewire.rs`
  - `src-tauri/src/platform/` - new directory with platform modules
  - `src-tauri/Cargo.toml` - platform-specific dependencies
- No frontend changes required
- No changes to audio processing, visualization, or transcription logic
