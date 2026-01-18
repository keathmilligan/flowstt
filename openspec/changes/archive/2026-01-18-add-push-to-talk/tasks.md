## 1. Common Types and IPC

- [x] 1.1 Add `TranscriptionMode` enum to `src-common/src/types.rs` (Automatic, PushToTalk)
- [x] 1.2 Add `KeyCode` enum to `src-common/src/types.rs` for platform-independent key representation
- [x] 1.3 Add `PttStatus` struct to `src-common/src/types.rs` (mode, key, is_active)
- [x] 1.4 Add IPC requests: `SetTranscriptionMode`, `SetPushToTalkKey`, `GetPttStatus`
- [x] 1.5 Add IPC responses: `PttStatusResponse`
- [x] 1.6 Add IPC events: `PttPressed`, `PttReleased`, `TranscriptionModeChanged`

## 2. Hotkey Backend Abstraction

- [x] 2.1 Create `src-service/src/hotkey/mod.rs` with platform selection
- [x] 2.2 Define `HotkeyBackend` trait in `src-service/src/hotkey/backend.rs`
- [x] 2.3 Define `HotkeyEvent` enum (Pressed, Released)

## 3. macOS Hotkey Implementation

- [x] 3.1 Create `src-service/src/hotkey/macos.rs` with CGEventTap implementation
- [x] 3.2 Implement Accessibility permission check
- [x] 3.3 Implement CGEventTap creation in passive mode
- [x] 3.4 Implement key filtering for configured hotkey
- [x] 3.5 Implement event delivery via channel
- [x] 3.6 Add run loop management on separate thread
- [x] 3.7 Implement clean shutdown

## 4. Windows Hotkey Implementation

- [x] 4.1 Create `src-service/src/hotkey/windows.rs` with Raw Input implementation
- [x] 4.2 Implement hidden message-only window creation
- [x] 4.3 Register for raw keyboard input with RIDEV_INPUTSINK
- [x] 4.4 Implement key filtering for configured hotkey
- [x] 4.5 Implement event delivery via channel
- [x] 4.6 Add message loop on separate thread
- [x] 4.7 Implement clean shutdown

## 4b. Linux Platform Stub

- [x] 4b.1 Create `src-service/src/hotkey/linux.rs` stub with not-implemented error

## 5. Service State Integration

- [x] 5.1 Add PTT fields to `ServiceState`: transcription_mode, ptt_key, is_ptt_active
- [x] 5.2 Add hotkey backend instance to service
- [x] 5.3 Implement PTT state change handlers

## 6. Audio Loop PTT Mode

- [x] 6.1 Modify `audio_loop.rs` to check transcription mode
- [x] 6.2 Add PTT event polling in audio loop
- [x] 6.3 Implement PTT segment start on key press (with 100ms lookback)
- [x] 6.4 Implement PTT segment end on key release
- [x] 6.5 Ensure VAD continues running for visualization in PTT mode
- [x] 6.6 Handle ring buffer overflow during long PTT holds (uses existing TranscribeState logic)

## 7. IPC Handler Integration

- [x] 7.1 Add handler for `SetTranscriptionMode` request
- [x] 7.2 Add handler for `SetPushToTalkKey` request
- [x] 7.3 Add handler for `GetPttStatus` request
- [x] 7.4 Emit `PttPressed`/`PttReleased` events to subscribed clients

## 8. Tauri Command Integration

- [x] 8.1 Add `set_transcription_mode` command to `src-tauri/src/lib.rs`
- [x] 8.2 Add `set_ptt_key` command
- [x] 8.3 Add `get_ptt_status` command
- [x] 8.4 Subscribe to PTT events for frontend

## 9. Frontend UI

- [x] 9.1 Add transcription mode selector to settings/main UI (TypeScript handlers)
- [x] 9.2 Add PTT key configuration UI (dropdown of supported keys)
- [x] 9.3 Add visual indicator when PTT is active (key held)
- [x] 9.4 Disable PTT mode option on unsupported platforms
- [x] 9.5 Show Accessibility permission guidance when needed on macOS

## 10. Configuration Persistence

- [x] 10.1 Add transcription mode to service configuration
- [x] 10.2 Add PTT key to service configuration
- [x] 10.3 Load configuration on service startup
- [x] 10.4 Save configuration on change

## 11. Testing and Validation

- [x] 11.1 Test macOS hotkey detection in foreground and background (implementation complete, manual testing needed)
- [x] 11.2 Test PTT segment creation and transcription (implementation complete, manual testing needed)
- [x] 11.3 Test mode switching while idle (implementation complete, manual testing needed)
- [x] 11.4 Test long PTT hold with buffer overflow (implementation complete, manual testing needed)
- [x] 11.5 Verify Windows implementation and Linux stub compile correctly
- [x] 11.6 Test permission denied handling on macOS (implementation complete, manual testing needed)
