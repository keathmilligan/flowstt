# Change: Add Spectrogram Display

## Why
The current audio visualization only shows amplitude over time (waveform). A spectrogram display showing frequency content would provide users with richer visual feedback about their audio input, helping identify speech characteristics, background noise, and audio quality issues.

## What Changes
- Add a new canvas element below the existing waveform to display a real-time spectrogram
- Implement FFT-based frequency analysis of incoming audio samples
- Render frequency bins as a scrolling color-mapped visualization
- Integrate with existing audio sample event pipeline

## Impact
- Affected specs: audio-visualization
- Affected code: 
  - `src/main.ts` - Add SpectrogramRenderer class, integrate with audio sample listener
  - `index.html` - Add spectrogram canvas element
  - `src/styles.css` - Style spectrogram container
