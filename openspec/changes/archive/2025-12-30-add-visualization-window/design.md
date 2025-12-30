# Design: Visualization Window with Mini Waveform

## Context
FlowSTT currently displays all audio visualizations (waveform, spectrogram, speech activity graph) in the main 800x600 window alongside controls and transcription output. This change separates visualizations into a dedicated resizable window while adding a compact mini waveform indicator in the main window header.

## Goals
- Provide a more compact main window focused on transcription
- Allow users to optionally view detailed visualizations in a separate window
- Maintain real-time audio feedback via mini waveform when full visualizations are hidden
- Support free-form window resizing for the visualization window

## Non-Goals
- Persisting window position/size between sessions (future enhancement)
- Making the visualization window always-on-top
- Adding additional visualization types

## Decisions

### Decision: Mini Waveform Implementation
Use a small HTML canvas element positioned next to the logo. Reuse the existing `WaveformRenderer` class logic but with a simplified rendering path that skips grid, labels, and axes.

**Rationale**: Maximizes code reuse while keeping the mini waveform lightweight. The existing ring buffer and animation loop infrastructure can be shared.

**Alternatives considered**:
- SVG-based waveform: More complex, harder to achieve smooth 60fps updates
- CSS-only animation: Cannot show actual audio data
- Separate mini renderer class: More code duplication

### Decision: Visualization Window as Tauri WebviewWindow
Create the visualization window as a second Tauri WebviewWindow that loads a separate HTML page (`visualization.html`).

**Rationale**: 
- Clean separation of concerns
- Window can be independently opened/closed/resized
- Tauri handles native window management
- Each window has its own DOM and rendering context

**Alternatives considered**:
- Single page with hidden/shown sections: Would keep all rendering active even when hidden
- iframe-based approach: Complicates event communication

### Decision: Event-Based Communication
The visualization window subscribes to the same `visualization-data` Tauri events as the main window currently does. The main window's mini waveform also subscribes to these events.

**Rationale**: No changes needed to the backend event emission. Both windows can independently listen and render.

### Decision: Main Window Compact Size
Reduce main window to approximately 800x300 pixels (width preserved, height reduced).

**Rationale**: Removing the visualization area (~300px) and speech activity graph (~150px) allows significant height reduction while preserving the transcription area and controls.

### Decision: Visualization Window Default State
The visualization window is closed by default. It opens when the user double-clicks the mini waveform.

**Rationale**: Keeps the UI minimal for users who don't need detailed visualizations. Power users can easily access them on demand.

## Architecture

```
Main Window (800x300)
+------------------------------------------+
| [Logo] [Mini Waveform~~~] [CUDA] [v0.1.0]|
+------------------------------------------+
| [Transcription text area                ]|
| [with fade effect                       ]|
+------------------------------------------+
| Status: Listening...          [Legend]   |
+------------------------------------------+
| [Source1 ▼] [Source2 ▼] [Mon][AEC][Mode] |
+------------------------------------------+

Visualization Window (800x600, resizable, min 800x600)
+------------------------------------------+
| [Waveform Canvas        ][Spectrogram   ]|
|                                          |
+------------------------------------------+
| [Speech Activity Canvas                 ]|
+------------------------------------------+
```

## Component Responsibilities

### MiniWaveformRenderer (new)
- Renders to a small canvas (~120x24 pixels)
- Receives same waveform data as full renderer
- Draws single gray line, no decorations
- Transparent background
- Handles double-click to open visualization window

### Visualization Window
- Contains WaveformRenderer, SpectrogramRenderer, SpeechActivityRenderer
- Subscribes to `visualization-data` events independently
- Resizable with 800x600 minimum
- No title bar (matches main window style)
- Can be closed independently

### Main Window
- Reduced height, no full visualization canvases
- Contains mini waveform in header
- All controls and transcription remain

## Event Flow

```
Backend emits visualization-data
         |
         +---> Main Window
         |          |
         |          +---> MiniWaveformRenderer (if monitoring)
         |
         +---> Visualization Window (if open)
                    |
                    +---> WaveformRenderer
                    +---> SpectrogramRenderer
                    +---> SpeechActivityRenderer
```

## Risks / Trade-offs

### Risk: Multiple event listeners for same data
**Mitigation**: Tauri events are broadcast; multiple listeners are efficient. No additional IPC overhead.

### Risk: User confusion about where visualizations went
**Mitigation**: Mini waveform provides visual indication that audio is active. Double-click affordance is discoverable.

### Trade-off: Two windows vs. one
**Accepted**: Provides flexibility. Users who want compact UI get it; users who want visualizations can open them.

## Migration Plan
1. Create visualization window infrastructure
2. Add mini waveform to main window header
3. Move visualization renderers to new window
4. Update main window layout and size
5. Wire up double-click to open visualization window

## Open Questions
- None currently
