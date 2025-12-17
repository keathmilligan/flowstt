# Change: Add Speech Activity Display

## Why
Users need visibility into how the speech detection algorithm is working to understand why speech is or isn't being detected. Currently, speech detection events are emitted but there's no visual feedback showing the underlying detection components.

## What Changes
- Add a new graphical display for speech activity below the waveform and spectrogram
- Display scrolls right-to-left matching the other visualizations
- Plot lines showing individual speech detection components (RMS amplitude, ZCR, spectral centroid, onset state, transient detection)
- Show speech detection state as a filled on/off bar
- Display threshold reference lines (-40dB voiced, -50dB whisper) as heavier grid lines
- Graph spans full width, ~20% height of other graphs

## Impact
- Affected specs: audio-visualization
- Affected code: 
  - `src/main.ts` (new SpeechActivityRenderer class, event handling)
  - `src-tauri/src/processor.rs` (emit speech detection metrics in visualization events)
  - `index.html` (new canvas element)
  - `src/styles.css` (styling for new graph area)
