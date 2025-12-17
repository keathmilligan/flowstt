## ADDED Requirements

### Requirement: Speech Activity Display
The system SHALL display a real-time speech activity visualization below the waveform and spectrogram, showing speech detection state and the underlying detection algorithm components.

#### Scenario: Speech activity graph renders during monitoring
- **WHEN** the user starts audio monitoring
- **THEN** a scrolling speech activity graph appears showing detection metrics over time, updating at 60fps

#### Scenario: Speech activity graph scrolls right to left
- **WHEN** new speech detection metrics arrive
- **THEN** they appear on the right edge of the display and scroll leftward as newer metrics arrive

#### Scenario: Speech activity graph renders during recording
- **WHEN** the user is recording audio
- **THEN** the speech activity visualization is active, showing detection metrics for audio being captured

#### Scenario: Speech activity graph clears on stop when not monitoring
- **WHEN** recording stops and monitoring was not active before recording started
- **THEN** the speech activity display shows an idle state (cleared canvas with background color)

#### Scenario: Speech activity graph continues on stop when monitoring was active
- **WHEN** recording stops and monitoring was active before recording started
- **THEN** the speech activity graph continues displaying live detection metrics

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
The system SHALL position the speech activity display below the existing visualizations with appropriate sizing.

#### Scenario: Full width display
- **WHEN** the speech activity graph renders
- **THEN** it spans the full width of the client area, matching the waveform and spectrogram

#### Scenario: Proportional height
- **WHEN** the speech activity graph renders
- **THEN** its height is approximately 20% of the height of the waveform and spectrogram graphs

## MODIFIED Requirements

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
