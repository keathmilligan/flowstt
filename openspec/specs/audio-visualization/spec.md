# audio-visualization Specification

## Purpose
TBD - created by archiving change add-live-audio-waveform. Update Purpose after archive.
## Requirements
### Requirement: Live Waveform Display
The system SHALL display a real-time waveform visualization of audio input in a dedicated canvas area.

#### Scenario: Waveform renders during monitoring
- **WHEN** the user starts audio monitoring
- **THEN** a scrolling waveform appears showing audio amplitude over time, updating at 60fps

#### Scenario: Waveform scrolls right to left
- **WHEN** new audio samples arrive
- **THEN** they appear on the right edge of the display and scroll leftward as newer samples arrive

#### Scenario: Waveform renders during recording
- **WHEN** the user is recording audio
- **THEN** the waveform visualization is active, showing the audio being captured

#### Scenario: Waveform clears on stop when not monitoring
- **WHEN** recording stops and monitoring was not active before recording started
- **THEN** the waveform display shows an idle state (flat line or cleared canvas)

#### Scenario: Waveform continues on stop when monitoring was active
- **WHEN** recording stops and monitoring was active before recording started
- **THEN** the waveform continues displaying live audio input

### Requirement: Audio Monitor Mode
The system SHALL allow the user to monitor audio input without recording, for verifying microphone function.

#### Scenario: Start monitoring
- **WHEN** the user clicks the Monitor button while idle
- **THEN** audio streaming begins, the waveform displays live input, and no audio is accumulated for transcription

#### Scenario: Stop monitoring
- **WHEN** the user clicks the Monitor button while monitoring
- **THEN** audio streaming stops and the waveform returns to idle state

#### Scenario: Recording starts while monitoring
- **WHEN** the user clicks Record while monitoring
- **THEN** recording begins seamlessly without disrupting the audio stream, and the waveform continues uninterrupted

#### Scenario: Recording stops while monitoring was active
- **WHEN** the user stops recording and monitoring was active before recording started
- **THEN** monitoring continues uninterrupted and the waveform keeps displaying live audio

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
The system SHALL display a real-time spectrogram visualization below the waveform, showing frequency content of audio input over time.

#### Scenario: Spectrogram renders during monitoring
- **WHEN** the user starts audio monitoring
- **THEN** a scrolling spectrogram appears showing frequency content over time, updating at 60fps

#### Scenario: Spectrogram scrolls right to left
- **WHEN** new audio samples arrive and are analyzed
- **THEN** new frequency data appears on the right edge of the display and scrolls leftward as newer data arrives

#### Scenario: Spectrogram renders during recording
- **WHEN** the user is recording audio
- **THEN** the spectrogram visualization is active, showing the frequency content of audio being captured

#### Scenario: Spectrogram clears on stop when not monitoring
- **WHEN** recording stops and monitoring was not active before recording started
- **THEN** the spectrogram display shows an idle state (cleared canvas with background color)

#### Scenario: Spectrogram continues on stop when monitoring was active
- **WHEN** recording stops and monitoring was active before recording started
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

