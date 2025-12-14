# Design: Spectrogram Display

## Context
The application currently displays a real-time amplitude waveform during audio monitoring/recording. Users have requested a spectrogram (frequency analysis) display to visualize the spectral content of audio, which provides complementary information to the amplitude display.

The spectrogram must:
- Run at 60fps alongside the existing waveform
- Process the same audio samples already being received via Tauri events
- Not impact audio capture or existing visualization performance

## Goals / Non-Goals

**Goals:**
- Display real-time frequency content as a scrolling spectrogram
- Use the existing audio sample event pipeline (no backend changes)
- Maintain 60fps rendering performance
- Provide clear visual distinction between frequency bands

**Non-Goals:**
- Audio analysis for speech detection (handled separately in Rust backend)
- Configurable FFT parameters (use sensible defaults)
- Logarithmic frequency scaling (linear is sufficient for MVP)
- Frequency axis labels (keep display minimal)

## Decisions

### Decision 1: FFT Implementation in JavaScript

**Choice**: Use the Web Audio API's AnalyserNode or a lightweight JavaScript FFT library.

**Alternatives considered:**
- **Web Audio AnalyserNode**: Provides built-in FFT but requires routing audio through Web Audio API, which we don't currently use since audio comes from Tauri backend events.
- **Rust-side FFT**: Would require modifying the backend to compute and emit frequency data, adding complexity and bandwidth.
- **JavaScript FFT library**: Process samples in the frontend using a library like `fft.js` or implement a simple radix-2 FFT.

**Rationale**: Implementing FFT in JavaScript keeps all changes frontend-only, aligns with the existing architecture where samples arrive via events, and avoids adding backend complexity. A simple FFT implementation or small library (~2KB) is sufficient for visualization purposes.

### Decision 2: FFT Window Size

**Choice**: Use a 512-sample FFT window.

**Rationale**: 
- At 48kHz sample rate, 512 samples = ~10.7ms window
- Provides 256 frequency bins (sufficient resolution for visualization)
- Balances frequency resolution vs. time resolution
- Matches well with the ~256 sample batches already being emitted

### Decision 3: Color Mapping

**Choice**: Use a heat map color scheme (dark blue -> cyan -> yellow -> red).

**Rationale**: Standard spectrogram convention that provides good visual separation between low and high energy frequency bins. Can be implemented with a simple gradient calculation.

### Decision 4: Rendering Approach

**Choice**: Render to an offscreen buffer, scroll by copying, then draw new column.

**Rationale**: 
- Scrolling spectrograms require shifting existing pixels left
- Using `getImageData`/`putImageData` for the scroll operation is efficient
- Only need to compute colors for one new column per frame
- Matches the approach used by professional audio tools

### Decision 5: Frequency Range

**Choice**: Display 0 Hz to Nyquist (sample_rate/2), typically 0-24kHz for 48kHz audio.

**Rationale**: Full range shows all captured audio content. Users can see speech (100-8000 Hz) as well as high-frequency content and noise.

## Component Architecture

```
Audio Samples Event
        |
        v
  +-----------+
  | main.ts   |
  +-----------+
        |
        +---> WaveformRenderer (existing)
        |
        +---> SpectrogramRenderer (new)
                    |
                    v
              +----------+
              | FFT      |
              | Processor|
              +----------+
                    |
                    v
              Canvas (new)
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| FFT computation may impact frame rate | Use efficient radix-2 algorithm; skip frames if behind |
| Memory usage from sample buffering | Use fixed-size circular buffer for FFT input |
| Color calculation per-pixel overhead | Pre-compute color lookup table |

## Open Questions

- Should the spectrogram height match the waveform or be smaller? (Suggest: equal height for visual balance)
- Should there be a toggle to hide/show the spectrogram? (Suggest: no, keep UI simple for MVP)
