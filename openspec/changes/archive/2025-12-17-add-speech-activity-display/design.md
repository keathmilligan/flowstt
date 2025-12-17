# Design: Speech Activity Display

## Context
The speech detector already computes multiple features for speech detection (RMS amplitude, ZCR, spectral centroid) and tracks state (onset detection, transient rejection). These values are currently only used internally for decision-making. This change exposes them visually to help users understand the detection algorithm's behavior.

## Goals
- Provide real-time visualization of speech detection components
- Help users understand why speech is/isn't detected
- Match visual style and scrolling behavior of existing waveform/spectrogram displays

## Non-Goals
- Adjustable detection parameters via UI (future enhancement)
- Historical analysis or playback of detection metrics

## Decisions

### Decision: Unified Visualization Event Extension
Extend the existing `VisualizationPayload` to include speech detection metrics rather than creating a separate event.

**Rationale**: Keeps all visualization data synchronized, reduces IPC overhead, and simplifies frontend event handling.

**Alternatives considered**:
- Separate `speech-metrics` event: Would require additional synchronization logic and increase IPC traffic
- Polling from frontend: Would add latency and complexity

### Decision: Line Graph for Detection Components
Use line graphs for continuous metrics (amplitude, ZCR, centroid) with distinct colors.

**Rationale**: Lines clearly show trends over time and allow multiple values to be compared simultaneously.

**Color scheme**:
- Amplitude (dB): Yellow/gold - primary detection metric
- ZCR: Cyan - mid-range visibility
- Spectral Centroid: Magenta - distinguishable from others
- Onset state: Green (voiced) / Blue (whisper) - semantic colors
- Transient detection: Red - warning/rejection indicator

### Decision: Filled Bar for Speech State
Speech detection state shown as a semi-transparent filled region at top of graph.

**Rationale**: Binary state is best shown as presence/absence, semi-transparency allows underlying lines to remain visible.

### Decision: Threshold Lines as Heavy Grid Lines
Display -40dB (voiced) and -50dB (whisper) thresholds as slightly heavier lines in the grid, labeled on Y-axis.

**Rationale**: Integrates thresholds into existing grid pattern, provides clear reference without cluttering the display.

### Decision: Normalized Y-Axis Ranges
Each metric type uses its own normalized range:
- Amplitude: -60dB to 0dB (log scale)
- ZCR: 0.0 to 0.5 (linear)
- Spectral Centroid: 0 to 8000 Hz (linear, covers speech range)

All normalized to 0-1 for rendering, with Y-axis showing actual units in labels.

## Data Flow

```
Audio Samples
     |
     v
SpeechDetector.process()
     |
     +-- Calculate RMS -> db
     +-- Calculate ZCR -> zcr  
     +-- Estimate Centroid -> centroid
     +-- Evaluate speech state -> is_speaking, mode
     +-- Check transient -> is_transient
     |
     v
VisualizationPayload {
    waveform: [...],
    spectrogram: Option<...>,
    speech_metrics: Option<SpeechMetrics> {  // NEW
        amplitude_db: f32,
        zcr: f32,
        centroid_hz: f32,
        is_speaking: bool,
        is_voiced_pending: bool,
        is_whisper_pending: bool,
        is_transient: bool,
    }
}
     |
     v
Frontend: SpeechActivityRenderer
     +-- Push metrics to ring buffers
     +-- Render lines for each metric
     +-- Render filled bar for speech state
     +-- Draw grid with threshold lines
```

## Risks / Trade-offs

- **Increased IPC payload size**: Adding ~28 bytes per event for speech metrics. Negligible given existing payload sizes.
- **Visual complexity**: Multiple lines may be overwhelming. Mitigated by distinct colors and option to add legend.
- **Performance**: Additional canvas rendering at 60fps. Mitigated by simple line drawing (no FFT, no pixel manipulation).

## Open Questions
- Should there be a toggle to show/hide individual metric lines? (Defer to future enhancement)
- Should the graph height be user-adjustable? (Defer to future enhancement)
