# Change: Add Separate Visualization Window with Mini Waveform

## Why
The current UI combines all visualizations (waveform, spectrogram, speech activity) in the main window, making it feel crowded. Users who want to focus on transcription don't need the full visualizations visible at all times, while power users who want detailed audio analysis need more space for visualizations.

## What Changes
- Move waveform, spectrogram, and speech activity visualizations to a new resizable window
- New visualization window is closed by default and opens on demand
- Add a mini waveform next to the logo in the main window header
- Mini waveform is a simplified, real-time visualization (no scale, labels, grid)
- Double-clicking the mini waveform opens the visualization window
- Main window becomes more compact without the full visualizations
- **BREAKING**: Main window size changes from 800x600 to a smaller compact size

## Impact
- Affected specs: `window-appearance`, `audio-visualization`
- Affected code: `src/main.ts`, `src/styles.css`, `index.html`, `src-tauri/tauri.conf.json`
