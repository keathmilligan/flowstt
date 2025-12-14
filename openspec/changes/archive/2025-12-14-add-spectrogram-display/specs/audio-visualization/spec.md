## ADDED Requirements

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
The system SHALL compute frequency content of audio samples using Fast Fourier Transform for spectrogram visualization.

#### Scenario: FFT window processing
- **WHEN** sufficient audio samples are buffered (512 samples)
- **THEN** the system performs FFT analysis and extracts magnitude for each frequency bin

#### Scenario: Frequency bins displayed
- **WHEN** FFT analysis completes
- **THEN** the resulting frequency magnitudes are mapped to colors and rendered as a vertical column in the spectrogram

### Requirement: Spectrogram Color Mapping
The system SHALL map frequency magnitude values to colors using a heat map gradient for visual clarity.

#### Scenario: Low energy displayed as cool colors
- **WHEN** a frequency bin has low magnitude
- **THEN** it is displayed in dark blue or black

#### Scenario: High energy displayed as warm colors
- **WHEN** a frequency bin has high magnitude
- **THEN** it is displayed in yellow, orange, or red

#### Scenario: Color gradient is continuous
- **WHEN** frequency magnitudes span the range from low to high
- **THEN** colors transition smoothly through the gradient (blue -> cyan -> green -> yellow -> red)
