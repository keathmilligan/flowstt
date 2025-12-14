## 1. Frontend Implementation

- [x] 1.1 Implement FFT processor class in `src/main.ts`
  - Radix-2 FFT algorithm (512-point)
  - Hanning window function
  - Magnitude extraction from complex output

- [x] 1.2 Implement SpectrogramRenderer class in `src/main.ts`
  - Canvas setup with HiDPI support
  - Sample buffer (ring buffer) for FFT input
  - Scrolling render via ImageData manipulation
  - Heat map color mapping function

- [x] 1.3 Integrate SpectrogramRenderer with audio sample listener
  - Push samples to spectrogram renderer alongside waveform
  - Start/stop spectrogram with monitoring state
  - Clear spectrogram on stop (when not continuing monitoring)

## 2. UI Layout

- [x] 2.1 Add spectrogram canvas to `index.html`
  - New canvas element below waveform
  - Container div for styling

- [x] 2.2 Update styles in `src/styles.css`
  - Spectrogram container styling
  - Canvas sizing and border
  - Adjust waveform/spectrogram height distribution

## 3. Validation

- [x] 3.1 Manual testing
  - Verify spectrogram renders during monitoring
  - Verify spectrogram renders during recording
  - Verify spectrogram clears when stopping (non-monitoring mode)
  - Verify spectrogram continues when stopping recording (monitoring was active)
  - Verify 60fps rendering performance
  - Verify speech produces visible frequency patterns
