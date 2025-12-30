# audio-visualization Specification

## Purpose
TBD - created by archiving change add-live-audio-waveform. Update Purpose after archive.
## Requirements
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

### Requirement: Audio Monitor Mode
The system SHALL allow the user to monitor audio input without recording, for verifying microphone function. When Transcribe mode is enabled, monitoring is implicitly active.

#### Scenario: Start monitoring
- **WHEN** the user clicks the Monitor button while idle
- **THEN** audio streaming begins, the waveform displays live input, and no audio is accumulated for transcription

#### Scenario: Stop monitoring
- **WHEN** the user clicks the Monitor button while monitoring
- **THEN** audio streaming stops and the waveform returns to idle state

#### Scenario: Transcribe mode activates monitoring
- **WHEN** the user enables Transcribe mode while monitoring is inactive
- **THEN** monitoring is automatically enabled and remains active while Transcribe is enabled

#### Scenario: Monitor toggle disabled when transcribe active
- **WHEN** Transcribe mode is active
- **THEN** the Monitor toggle is disabled (monitoring is implicitly on)

#### Scenario: Stopping monitoring stops transcribe
- **WHEN** the user attempts to stop monitoring while Transcribe mode is active
- **THEN** both monitoring and Transcribe mode are disabled

### Requirement: Low-Latency Audio Streaming
The system SHALL stream render-ready visualization data from the backend to the frontend with minimal latency for real-time display.

#### Scenario: Visualization data delivered via events
- **WHEN** audio is being captured (monitoring or recording)
- **THEN** pre-computed visualization data is emitted to the frontend in batches via Tauri events containing waveform amplitudes and spectrogram colors

#### Scenario: Visualization latency is imperceptible
- **WHEN** audio input occurs (e.g., user taps microphone)
- **THEN** the waveform and spectrogram reflect the input within one display frame (~16ms), appearing instantaneous to the user

#### Scenario: Stop recording does not disrupt waveform
- **WHEN** the user stops recording while monitoring was active
- **THEN** the waveform and spectrogram continue displaying without any visual disruption or pause

### Requirement: Non-Blocking Transcription
The system SHALL process and transcribe recorded audio in the background without blocking the UI or audio stream.

#### Scenario: Recording stops immediately
- **WHEN** the user clicks stop recording
- **THEN** the recording stops immediately and UI is responsive

#### Scenario: Transcription runs in background
- **WHEN** recording stops
- **THEN** audio processing and transcription run in a background thread

#### Scenario: Transcription results delivered via events
- **WHEN** transcription completes
- **THEN** the result is delivered to the frontend via a Tauri event

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

### Requirement: FFT-Based Frequency Analysis
The system SHALL compute frequency content of audio samples using Fast Fourier Transform in the backend for spectrogram visualization.

#### Scenario: FFT window processing
- **WHEN** sufficient audio samples are buffered (512 samples)
- **THEN** the backend performs FFT analysis and extracts magnitude for each frequency bin

#### Scenario: Frequency bins mapped to colors
- **WHEN** FFT analysis completes
- **THEN** the backend maps frequency magnitudes to RGB colors and emits them as a spectrogram column ready for direct rendering

### Requirement: Spectrogram Color Mapping
The system SHALL map frequency magnitude values to colors using a heat map gradient in the backend for visual clarity.

#### Scenario: Low energy displayed as cool colors
- **WHEN** a frequency bin has low magnitude
- **THEN** the backend emits dark blue or black RGB values

#### Scenario: High energy displayed as warm colors
- **WHEN** a frequency bin has high magnitude
- **THEN** the backend emits yellow, orange, or red RGB values

#### Scenario: Color gradient is continuous
- **WHEN** frequency magnitudes span the range from low to high
- **THEN** colors transition smoothly through the gradient (blue -> cyan -> green -> yellow -> red)

### Requirement: Backend Waveform Processing
The system SHALL compute pre-downsampled waveform amplitude values in the backend, ready for direct rendering by the frontend.

#### Scenario: Waveform downsampling
- **WHEN** audio samples are captured
- **THEN** the backend downsamples them using peak detection to produce render-ready amplitude values

#### Scenario: Waveform data emitted with visualization events
- **WHEN** visualization data is emitted
- **THEN** waveform amplitudes are included in every event for continuous display updates

### Requirement: Unified Visualization Event
The system SHALL emit a single event type containing waveform, spectrogram, and speech detection metrics data to minimize IPC overhead.

#### Scenario: Combined payload structure
- **WHEN** visualization data is ready to emit
- **THEN** the system sends a `visualization-data` event containing waveform amplitudes, an optional spectrogram column, and optional speech detection metrics

#### Scenario: Spectrogram column included when ready
- **WHEN** the FFT buffer fills (every 512 samples)
- **THEN** the visualization event includes a spectrogram column with RGB color data

#### Scenario: Waveform-only events between FFT frames
- **WHEN** visualization data is emitted before the FFT buffer is full
- **THEN** the event contains waveform data but no spectrogram column

#### Scenario: Speech metrics included with every event
- **WHEN** visualization data is emitted
- **THEN** the event includes speech detection metrics (amplitude_db, zcr, centroid_hz, is_speaking, onset states, is_transient)

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

### Requirement: Speech Detection Metrics Visualization
The system SHALL display individual speech detection algorithm components as colored line graphs within the speech activity display.

#### Scenario: Amplitude line displayed
- **WHEN** speech detection metrics are received
- **THEN** the RMS amplitude (in dB) is plotted as a gold/yellow line

#### Scenario: Zero-crossing rate line displayed
- **WHEN** speech detection metrics are received
- **THEN** the zero-crossing rate is plotted as a cyan line

#### Scenario: Spectral centroid line displayed
- **WHEN** speech detection metrics are received
- **THEN** the spectral centroid (in Hz) is plotted as a magenta line

#### Scenario: Onset state indicators displayed
- **WHEN** speech detection metrics are received
- **THEN** voiced onset pending state is indicated with a green marker and whisper onset pending state with a blue marker

#### Scenario: Transient detection indicator displayed
- **WHEN** a transient sound is detected (keyboard click, etc.)
- **THEN** the transient state is indicated with a red marker

### Requirement: Speech Detection State Bar
The system SHALL display the current speech detection state as a filled bar indicator that allows underlying metric lines to remain visible.

#### Scenario: Speech state bar fills when speaking
- **WHEN** the speech detector determines speech is occurring
- **THEN** a semi-transparent filled region appears at the top of the graph indicating active speech

#### Scenario: Speech state bar clears when silent
- **WHEN** the speech detector determines speech has ended
- **THEN** the filled region disappears, showing only the background

#### Scenario: Underlying lines visible through speech bar
- **WHEN** the speech state bar is active
- **THEN** the metric lines beneath it remain visible through the semi-transparent fill

### Requirement: Speech Detection Threshold Lines
The system SHALL display speech detection thresholds as reference lines in the speech activity graph grid.

#### Scenario: Voiced threshold displayed
- **WHEN** the speech activity graph renders
- **THEN** a heavier grid line is drawn at the -40dB level (voiced speech threshold) with label

#### Scenario: Whisper threshold displayed
- **WHEN** the speech activity graph renders
- **THEN** a heavier grid line is drawn at the -50dB level (whisper speech threshold) with label

#### Scenario: Threshold lines distinguished from regular grid
- **WHEN** the speech activity graph renders grid lines
- **THEN** threshold lines are visually heavier/more prominent than regular grid lines

### Requirement: Speech Activity Display Layout
The system SHALL position the speech activity display below the existing visualizations in the visualization window with appropriate sizing.

#### Scenario: Full width display
- **WHEN** the speech activity graph renders in the visualization window
- **THEN** it spans the full width of the visualization window client area, matching the waveform and spectrogram

#### Scenario: Proportional height
- **WHEN** the speech activity graph renders in the visualization window
- **THEN** its height is approximately 20% of the height of the waveform and spectrogram graphs

### Requirement: Lookback Speech Visualization
The system SHALL visually distinguish lookback-determined speech regions from confirmed speech regions in the speech activity graph, using different colors to show where speech actually started versus when it was confirmed.

#### Scenario: Lookback speech displayed with distinct color
- **WHEN** speech is confirmed and lookback determines an earlier start point
- **THEN** the region between the lookback start and confirmation point is displayed in a distinct lookback color

#### Scenario: Confirmed speech displayed with standard color
- **WHEN** speech is confirmed
- **THEN** the region from confirmation onward is displayed in the standard speech color

#### Scenario: Both regions visible simultaneously
- **WHEN** the speech activity graph scrolls
- **THEN** both lookback and confirmed speech regions are visible, showing the temporal relationship between true start and confirmation

#### Scenario: Lookback region precedes confirmed region
- **WHEN** speech is visualized in the delayed graph
- **THEN** the lookback-colored region appears to the left of (earlier than) the confirmed speech region

### Requirement: Speech Activity Delay Buffer
The system SHALL buffer speech detection metrics for the lookback duration before rendering, enabling retroactive insertion of lookback speech state at the correct temporal position.

#### Scenario: Metrics buffered before rendering
- **WHEN** speech detection metrics are received
- **THEN** they are held in a delay buffer for 200ms before being rendered to the canvas

#### Scenario: Lookback state inserted retroactively
- **WHEN** speech is confirmed with a lookback offset
- **THEN** the lookback speech state is inserted into the delay buffer at the position corresponding to the true speech start

#### Scenario: Buffer maintains temporal ordering
- **WHEN** metrics are rendered from the delay buffer
- **THEN** they are rendered in chronological order with lookback insertions at correct positions

### Requirement: Streaming Transcription Display
The system SHALL display transcription output as a continuously streaming text panel that appends new text to the existing content rather than replacing it entirely.

#### Scenario: Text appends on new transcription
- **WHEN** new transcription text is received from the speech-to-text engine
- **THEN** the text is appended to the end of the existing displayed text with a space separator

#### Scenario: No line breaks between segments
- **WHEN** multiple transcription segments are received
- **THEN** they are joined as continuous flowing text without line breaks between them

#### Scenario: Auto-scroll to newest text
- **WHEN** new text is appended to the transcription display
- **THEN** the display automatically scrolls to show the most recent text at the bottom

### Requirement: Transcription Text Styling
The system SHALL display transcription text using a light blue color and fixed-width font for readability.

#### Scenario: Fixed-width font rendering
- **WHEN** transcription text is displayed
- **THEN** it renders using the bundled Fira Mono font

#### Scenario: Light blue text color
- **WHEN** transcription text is displayed
- **THEN** the text color is light blue (#7DD3FC) providing good contrast against the dark background

### Requirement: Transcription Panel Fade Effect
The system SHALL display a fade-out gradient at the top of the transcription panel to indicate content continues above the visible area.

#### Scenario: Top fade gradient visible
- **WHEN** the transcription panel contains text
- **THEN** a gradient fade effect is visible at the top edge, transitioning from transparent to fully visible over approximately 15% of the panel height

#### Scenario: Fade does not obscure recent text
- **WHEN** text is displayed in the panel
- **THEN** the most recent text at the bottom of the panel is fully visible without any fade effect

### Requirement: Transcription Buffer Limit
The system SHALL limit the transcription display to the most recent text that fits within the panel, discarding older content.

#### Scenario: Old text discarded when buffer full
- **WHEN** the accumulated transcription text exceeds the visible capacity of the panel
- **THEN** the oldest text is removed from the beginning to keep only the most recent content visible

#### Scenario: No scrollable history
- **WHEN** the user attempts to scroll the transcription panel
- **THEN** no additional history is available beyond what is currently visible

### Requirement: Bundled Transcription Font
The system SHALL bundle the Fira Mono font with the application for consistent transcription text rendering across platforms.

#### Scenario: Font loads from application bundle
- **WHEN** the application starts
- **THEN** the Fira Mono font is loaded from the bundled font files

#### Scenario: Font available offline
- **WHEN** the application runs without network connectivity
- **THEN** the transcription font renders correctly from the local bundle

### Requirement: Transcribe Toggle Control
The system SHALL display a Transcribe toggle switch in the controls bar that enables or disables automatic speech-triggered transcription mode.

#### Scenario: Transcribe toggle displayed
- **WHEN** the application loads
- **THEN** a Transcribe toggle switch is visible in the control buttons area

#### Scenario: Transcribe toggle enables transcription mode
- **WHEN** the user enables the Transcribe toggle
- **THEN** monitoring starts (if not already active), speech detection triggers recording, and the toggle shows active state

#### Scenario: Transcribe toggle disables transcription mode
- **WHEN** the user disables the Transcribe toggle
- **THEN** speech-triggered recording stops and any in-progress segment is finalized

#### Scenario: Transcribe toggle disabled without source
- **WHEN** no audio source is selected
- **THEN** the Transcribe toggle is disabled

### Requirement: Transcribe Mode Status Display
The system SHALL display status messages that reflect the current transcribe mode state and activity.

#### Scenario: Status shows listening
- **WHEN** transcribe mode is active and idle (not currently capturing speech)
- **THEN** the status displays "Listening..."

#### Scenario: Status shows speech capture in progress
- **WHEN** transcribe mode is active and speech is being captured
- **THEN** the status displays "Recording speech..."

#### Scenario: Status shows transcription pending
- **WHEN** segments are queued for transcription
- **THEN** the status indicates pending transcriptions (e.g., "Transcribing... (2 pending)")

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

