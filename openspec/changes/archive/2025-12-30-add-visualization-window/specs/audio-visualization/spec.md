## MODIFIED Requirements

### Requirement: Live Waveform Display
The system SHALL display a real-time waveform visualization of audio input in a dedicated canvas area within the visualization window.

#### Scenario: Waveform renders during monitoring
- **WHEN** the user starts audio monitoring and the visualization window is open
- **THEN** a scrolling waveform appears showing audio amplitude over time, updating at 60fps

#### Scenario: Waveform scrolls right to left
- **WHEN** new audio samples arrive and the visualization window is open
- **THEN** they appear on the right edge of the display and scroll leftward as newer samples arrive

#### Scenario: Waveform renders during recording
- **WHEN** the user is recording audio and the visualization window is open
- **THEN** the waveform visualization is active, showing the audio being captured

#### Scenario: Waveform clears on stop when not monitoring
- **WHEN** recording stops, monitoring was not active before recording started, and the visualization window is open
- **THEN** the waveform display shows an idle state (flat line or cleared canvas)

#### Scenario: Waveform continues on stop when monitoring was active
- **WHEN** recording stops, monitoring was active before recording started, and the visualization window is open
- **THEN** the waveform continues displaying live audio input

#### Scenario: Visualization window closed does not receive waveform updates
- **WHEN** the visualization window is closed
- **THEN** no waveform rendering occurs in that window (resources not consumed)

### Requirement: Spectrogram Display
The system SHALL display a real-time spectrogram visualization below the waveform in the visualization window, showing frequency content of audio input over time.

#### Scenario: Spectrogram renders during monitoring
- **WHEN** the user starts audio monitoring and the visualization window is open
- **THEN** a scrolling spectrogram appears showing frequency content over time, updating at 60fps

#### Scenario: Spectrogram scrolls right to left
- **WHEN** new audio samples arrive, are analyzed, and the visualization window is open
- **THEN** new frequency data appears on the right edge of the display and scrolls leftward as newer data arrives

#### Scenario: Spectrogram renders during recording
- **WHEN** the user is recording audio and the visualization window is open
- **THEN** the spectrogram visualization is active, showing the frequency content of audio being captured

#### Scenario: Spectrogram clears on stop when not monitoring
- **WHEN** recording stops, monitoring was not active before recording started, and the visualization window is open
- **THEN** the spectrogram display shows an idle state (cleared canvas with background color)

#### Scenario: Spectrogram continues on stop when monitoring was active
- **WHEN** recording stops, monitoring was active before recording started, and the visualization window is open
- **THEN** the spectrogram continues displaying live frequency content

### Requirement: Speech Activity Display
The system SHALL display a real-time speech activity visualization below the waveform and spectrogram in the visualization window, showing speech detection state and the underlying detection algorithm components. The speech activity graph SHALL be delayed by the lookback duration (200ms) to allow lookback-determined speech starts to be displayed at their correct temporal position.

#### Scenario: Speech activity graph renders during monitoring
- **WHEN** the user starts audio monitoring and the visualization window is open
- **THEN** a scrolling speech activity graph appears showing detection metrics over time, updating at 60fps

#### Scenario: Speech activity graph scrolls right to left
- **WHEN** new speech detection metrics arrive and the visualization window is open
- **THEN** they appear on the right edge of the display and scroll leftward as newer metrics arrive

#### Scenario: Speech activity graph renders during recording
- **WHEN** the user is recording audio and the visualization window is open
- **THEN** the speech activity visualization is active, showing detection metrics for audio being captured

#### Scenario: Speech activity graph clears on stop when not monitoring
- **WHEN** recording stops, monitoring was not active before recording started, and the visualization window is open
- **THEN** the speech activity display shows an idle state (cleared canvas with background color)

#### Scenario: Speech activity graph continues on stop when monitoring was active
- **WHEN** recording stops, monitoring was active before recording started, and the visualization window is open
- **THEN** the speech activity graph continues displaying live detection metrics

#### Scenario: Speech activity graph is delayed
- **WHEN** speech detection metrics are received and the visualization window is open
- **THEN** they are buffered for 200ms before being rendered, allowing lookback results to be inserted at the correct position

#### Scenario: Waveform and spectrogram remain real-time
- **WHEN** audio is being monitored and the visualization window is open
- **THEN** the waveform and spectrogram display with minimal latency while the speech activity graph is intentionally delayed

### Requirement: Speech Activity Display Layout
The system SHALL position the speech activity display below the existing visualizations in the visualization window with appropriate sizing.

#### Scenario: Full width display
- **WHEN** the speech activity graph renders in the visualization window
- **THEN** it spans the full width of the visualization window client area, matching the waveform and spectrogram

#### Scenario: Proportional height
- **WHEN** the speech activity graph renders in the visualization window
- **THEN** its height is approximately 20% of the height of the waveform and spectrogram graphs

## ADDED Requirements

### Requirement: Mini Waveform Rendering
The system SHALL render a simplified real-time waveform in the mini waveform canvas that shows audio activity without detailed metrics.

#### Scenario: Mini waveform receives visualization data
- **WHEN** visualization data events are emitted during monitoring
- **THEN** the mini waveform renderer receives and processes the waveform amplitude data

#### Scenario: Mini waveform draws gray line
- **WHEN** the mini waveform renders audio data
- **THEN** it draws the waveform as a gray (#888888) line without glow effects

#### Scenario: Mini waveform has transparent background
- **WHEN** the mini waveform renders
- **THEN** the canvas background is transparent, allowing the header background to show through

#### Scenario: Mini waveform has no decorations
- **WHEN** the mini waveform renders
- **THEN** no grid lines, axis labels, scale indicators, or margins are drawn

#### Scenario: Mini waveform matches full waveform time window
- **WHEN** the mini waveform renders
- **THEN** it displays the same ~80ms time window as the full waveform visualization

#### Scenario: Mini waveform scrolls right to left
- **WHEN** new audio samples arrive
- **THEN** they appear on the right edge of the mini waveform and scroll leftward

#### Scenario: Mini waveform updates at 60fps
- **WHEN** audio monitoring is active
- **THEN** the mini waveform animation loop runs at 60fps for smooth visualization

#### Scenario: Mini waveform idle state
- **WHEN** audio monitoring is not active
- **THEN** the mini waveform displays a flat horizontal gray line at center height

### Requirement: Visualization Window Event Subscription
The visualization window SHALL independently subscribe to visualization data events from the backend.

#### Scenario: Visualization window subscribes on open
- **WHEN** the visualization window opens
- **THEN** it establishes its own listener for `visualization-data` events

#### Scenario: Visualization window unsubscribes on close
- **WHEN** the visualization window closes
- **THEN** it removes its event listener to prevent resource leaks

#### Scenario: Both windows receive events simultaneously
- **WHEN** visualization data is emitted and both the main window (mini waveform) and visualization window are active
- **THEN** both receive the event and render independently

### Requirement: Visualization Window Close Button
The visualization window SHALL display a close button to allow the user to close it.

#### Scenario: Close button visible
- **WHEN** the visualization window is open
- **THEN** a close button is visible in the top-right corner of the window

#### Scenario: Close button closes window
- **WHEN** the user clicks the close button
- **THEN** the visualization window closes

#### Scenario: Close button style matches main window
- **WHEN** the visualization window renders
- **THEN** the close button has the same appearance as the main window close button
