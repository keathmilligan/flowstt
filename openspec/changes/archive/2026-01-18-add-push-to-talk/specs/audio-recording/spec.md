## ADDED Requirements

### Requirement: Transcription Mode Selection
The system SHALL support multiple transcription modes that determine how speech segment boundaries are identified. The available modes are "Automatic" (VAD-triggered) and "Push-to-Talk" (hotkey-triggered).

#### Scenario: Automatic mode (default)
- **WHEN** transcription mode is set to Automatic
- **THEN** speech segments are delimited by Voice Activity Detection events (existing behavior)

#### Scenario: Push-to-Talk mode
- **WHEN** transcription mode is set to Push-to-Talk
- **THEN** speech segments are delimited by hotkey press and release events

#### Scenario: Mode persists across sessions
- **WHEN** the user selects a transcription mode
- **THEN** the mode preference is saved and restored on next application launch

#### Scenario: Mode change while transcribing
- **WHEN** the user attempts to change transcription mode while transcribe is active
- **THEN** the change is rejected until transcribe is stopped

### Requirement: Push-to-Talk Segment Control
The system SHALL start a speech segment when the PTT hotkey is pressed and end the segment when the hotkey is released, bypassing normal VAD-based segment boundaries.

#### Scenario: PTT key pressed starts segment
- **WHEN** transcription mode is Push-to-Talk AND the user presses the PTT hotkey
- **THEN** audio capture marks the segment start position immediately

#### Scenario: PTT key released ends segment
- **WHEN** a PTT segment is in progress AND the user releases the PTT hotkey
- **THEN** the segment is immediately finalized, saved to WAV, and queued for transcription

#### Scenario: PTT respects lookback
- **WHEN** a PTT segment starts
- **THEN** a small lookback buffer (100ms) is included to capture speech onset that precedes the physical key press

#### Scenario: PTT segment has no automatic duration limits
- **WHEN** a PTT segment is in progress
- **THEN** no automatic segmentation occurs at word breaks or duration thresholds (user controls duration via key hold)

#### Scenario: PTT segment respects ring buffer limits
- **WHEN** a PTT segment approaches ring buffer capacity (90% full)
- **THEN** the segment is automatically split and queued, with a new segment continuing from the current position

#### Scenario: VAD continues for visualization in PTT mode
- **WHEN** transcription mode is Push-to-Talk
- **THEN** speech detection still runs and emits metrics for visualization, but does not trigger segment boundaries

### Requirement: Push-to-Talk State Events
The system SHALL emit events when push-to-talk state changes, enabling UI feedback about PTT activation.

#### Scenario: PTT pressed event
- **WHEN** the PTT hotkey is pressed in Push-to-Talk mode
- **THEN** the system emits a `ptt-pressed` event to subscribed clients

#### Scenario: PTT released event
- **WHEN** the PTT hotkey is released in Push-to-Talk mode
- **THEN** the system emits a `ptt-released` event to subscribed clients

#### Scenario: Events include timestamp
- **WHEN** a PTT event is emitted
- **THEN** the event includes a timestamp for correlation with audio segments

### Requirement: Push-to-Talk Requires Transcribe Active
The system SHALL only respond to PTT hotkey events when transcribe mode is active, preventing accidental audio capture.

#### Scenario: PTT ignored when transcribe inactive
- **WHEN** the user presses the PTT hotkey but transcribe toggle is off
- **THEN** no segment is started and no audio is captured

#### Scenario: PTT works when transcribe active
- **WHEN** the user presses the PTT hotkey with transcribe toggle on
- **THEN** segment capture begins as normal
