# Tasks: Add Speech Activity Display

## 1. Backend: Emit Speech Detection Metrics
- [x] 1.1 Define `SpeechMetrics` struct in `processor.rs` with amplitude_db, zcr, centroid_hz, is_speaking, is_voiced_pending, is_whisper_pending, is_transient fields
- [x] 1.2 Add `speech_metrics: Option<SpeechMetrics>` field to `VisualizationPayload`
- [x] 1.3 Modify `SpeechDetector` to expose computed metrics via a getter method
- [x] 1.4 Update audio callback to collect speech metrics and include in visualization events

## 2. Frontend: Speech Activity Renderer
- [x] 2.1 Add `SpeechMetrics` TypeScript interface matching backend struct
- [x] 2.2 Update `VisualizationPayload` interface to include optional `speech_metrics` field
- [x] 2.3 Create `SpeechActivityRenderer` class with ring buffers for each metric
- [x] 2.4 Implement `pushMetrics()` method to store incoming metric values
- [x] 2.5 Implement `draw()` method with grid, threshold lines, metric lines, and speech state bar
- [x] 2.6 Implement color scheme for different metrics (amplitude: gold, ZCR: cyan, centroid: magenta, etc.)
- [x] 2.7 Add threshold lines (-40dB, -50dB) as heavier grid lines with labels

## 3. Frontend: UI Integration
- [x] 3.1 Add `<canvas id="speech-activity-canvas">` to `index.html` below spectrogram
- [x] 3.2 Add `.speech-activity-area` CSS styles (full width, ~20% height of other graphs)
- [x] 3.3 Initialize `SpeechActivityRenderer` in `main.ts` DOMContentLoaded handler
- [x] 3.4 Update visualization event listener to pass speech metrics to renderer
- [x] 3.5 Wire up start/stop/clear methods matching waveform and spectrogram lifecycle
- [x] 3.6 Handle window resize for speech activity canvas

## 4. Validation
- [x] 4.1 Verify graph scrolls right-to-left in sync with waveform/spectrogram
- [x] 4.2 Verify speech detection bar fills when speaking, clears when silent
- [x] 4.3 Verify threshold lines appear at correct positions with heavier weight
- [x] 4.4 Verify all metric lines are visible and distinguishable
- [x] 4.5 Test with various audio inputs (speech, whisper, keyboard clicks, silence)
