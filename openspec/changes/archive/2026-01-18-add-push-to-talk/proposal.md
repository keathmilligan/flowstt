# Change: Add Push-to-Talk Operating Mode

## Why

The current transcription system operates in automatic mode where speech detection (VAD) determines when to start and stop recording. While this works well for hands-free scenarios, many users prefer explicit control over when audio is captured—particularly in noisy environments, during meetings where they only want to capture their own intentional speech, or when using push-to-talk patterns familiar from gaming and communication applications.

Push-to-talk provides deterministic behavior: recording starts exactly when the user presses a key and stops when released, eliminating false triggers from background noise and giving users precise control over what gets transcribed.

## What Changes

- **New transcription mode**: Add a "Push-to-Talk" mode alongside the existing "Automatic" (VAD-based) mode
- **Global hotkey system**: Implement platform-specific global hotkey capture that works when the application window is not focused
- **macOS implementation**: Full implementation using CGEventTap API for system-wide key monitoring
- **Windows implementation**: Full implementation using Raw Input API for system-wide key monitoring
- **Platform stubs**: Stub implementation for Linux (X11/XCB) to be completed later
- **Configuration**: Configurable hotkey (default: Right Alt key)
- **UI integration**: Mode selection in the GUI and visual feedback when PTT is active
- **IPC extensions**: New request/response types for PTT configuration and state

## Impact

- Affected specs:
  - `audio-recording` - Modified to support PTT mode as alternative to VAD
  - `hotkey-input` - New capability for global hotkey capture

- Affected code:
  - `src-common/src/types.rs` - Add `TranscriptionMode` enum
  - `src-common/src/ipc/requests.rs` - Add PTT configuration requests
  - `src-common/src/ipc/responses.rs` - Add PTT events
  - `src-service/src/state.rs` - Add PTT state fields
  - `src-service/src/hotkey/` - New module for platform-specific hotkey capture
  - `src-service/src/hotkey/macos.rs` - macOS CGEventTap implementation
  - `src-service/src/hotkey/windows.rs` - Windows Raw Input implementation
  - `src-service/src/audio_loop.rs` - Modify to respect PTT mode
  - `src-service/src/ipc/handlers.rs` - Handle PTT requests
  - `src-tauri/src/lib.rs` - Expose PTT commands to frontend
  - `src/main.ts` - UI for mode selection and PTT state display
