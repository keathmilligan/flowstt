# Tasks: Add Visualization Window with Mini Waveform

## 1. Visualization Window Infrastructure
- [x] 1.1 Create `visualization.html` page with waveform, spectrogram, and speech activity canvases
- [x] 1.2 Create `visualization.ts` entry point that initializes renderers and subscribes to events
- [x] 1.3 Create `visualization.css` with styles for the visualization window layout
- [x] 1.4 Update `vite.config.ts` to build visualization page as additional entry point
- [ ] 1.5 Update `tauri.conf.json` to register visualization window configuration (min size 800x600, resizable, no decorations)

## 2. Mini Waveform Component
- [x] 2.1 Add mini waveform canvas element to main window header (next to logo)
- [x] 2.2 Create `MiniWaveformRenderer` class with simplified rendering (gray line, transparent bg, no grid/labels)
- [x] 2.3 Wire mini waveform to receive `visualization-data` events
- [x] 2.4 Add double-click handler on mini waveform to open visualization window
- [x] 2.5 Style mini waveform canvas (proportional to logo height, vertically aligned)

## 3. Window Management
- [x] 3.1 Add function to open visualization window via Tauri WebviewWindow API
- [x] 3.2 Track visualization window state (open/closed) in main window
- [x] 3.3 Ensure visualization window can be closed independently
- [x] 3.4 Handle case where user double-clicks mini waveform when window already open (focus existing)

## 4. Main Window Layout Updates
- [x] 4.1 Remove visualization container from main window HTML
- [x] 4.2 Update main window CSS for compact layout
- [x] 4.3 Update `tauri.conf.json` main window size to compact dimensions (~800x300)
- [x] 4.4 Adjust transcription area to use available space
- [x] 4.5 Move speech activity legend to visualization window (or remove from main)

## 5. Code Migration
- [x] 5.1 Move `WaveformRenderer`, `SpectrogramRenderer`, `SpeechActivityRenderer` classes to shared module
- [x] 5.2 Update main.ts to remove full visualization renderer initialization
- [x] 5.3 Update visualization.ts to initialize renderers with existing classes
- [x] 5.4 Ensure event listeners are properly cleaned up when visualization window closes

## 6. Testing & Validation
- [ ] 6.1 Verify mini waveform animates in real-time during monitoring
- [ ] 6.2 Verify double-click opens visualization window
- [ ] 6.3 Verify visualization window shows all three visualizations correctly
- [ ] 6.4 Verify visualization window is resizable with 800x600 minimum
- [ ] 6.5 Verify closing visualization window doesn't affect main window or audio
- [ ] 6.6 Verify main window layout is compact and usable
- [ ] 6.7 Build and test on Windows
