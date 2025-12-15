<picture>
  <source srcset="assets/flowstt-landscape.svg" media="(prefers-color-scheme: dark)">
  <source srcset="assets/flowstt-landscape-light.svg" media="(prefers-color-scheme: light)">
  <img src="assets/flowstt-landscape.svg" alt="FlowSTT logo">
</picture>

A voice transcription agent for fluid, natural conversation. FlowSTT goes beyond simple speech-to-text with real-time cadence analysis, intelligent speech detection, and rich automation capabilities.

![FlowSTT Demo](assets/flowstt.gif)

## Vision

Traditional voice assistants feel robotic. FlowSTT treats conversation as a continuous, adaptive stream where timing, cadence, and intent all matter. The system knows *when* to respond, not just *what* to respond to.

## Current Features

- **Audio Capture**: Microphone, system audio (loopback), or mixed mode
- **Real-time Visualization**: Live waveform and spectrogram displays
- **Speech Detection**: Multi-feature analysis (amplitude, ZCR, spectral centroid) with voiced/whisper modes
- **Local Transcription**: Offline Whisper inference via whisper-rs
- **Transient Rejection**: Filters keyboard clicks, mouse sounds, and ambient noise

## Roadmap

- [x] Audio device enumeration and selection
- [x] Audio recording with format conversion (16kHz mono)
- [x] Local Whisper transcription
- [x] Live waveform visualization (60fps)
- [x] Audio monitor mode (preview without recording)
- [x] Voice processing toggle with extensible processor architecture
- [x] Speech detection events (speech-started/speech-ended)
- [x] Enhanced speech detection (ZCR, spectral centroid, transient rejection)
- [x] Spectrogram display with FFT analysis
- [x] Backend visualization processing (unified event pipeline)
- [x] System audio capture (PipeWire/PulseAudio monitor sources)
- [x] Mixed audio capture (mic + system combined)
- [ ] Real-time cadence analysis (natural pause vs end-of-thought detection)
- [ ] Adaptive timeout management (context-aware listening windows)
- [ ] Acknowledgment feedback loop (accept tone, processing indicator)
- [ ] Interrupt handling (soft/hard interrupts with recovery)
- [ ] Dynamic query & follow-up behavior (clarifying questions)
- [ ] Multi-modal input (voice + CLI + gestures)
- [ ] Workflow automation (action execution from voice commands)

## Prerequisites

### Whisper Model

Download a model from [whisper.cpp models](https://huggingface.co/ggerganov/whisper.cpp/tree/main) and place it at:
- **Linux**: `~/.cache/whisper/ggml-base.en.bin`
- **macOS**: `~/Library/Caches/whisper/ggml-base.en.bin`
- **Windows**: `C:\Users\<username>\AppData\Local\whisper\ggml-base.en.bin`

### Build Dependencies

- Rust, Node.js, pnpm, CMake, C/C++ compiler
- **Linux**: `libasound2-dev` (Debian/Ubuntu) or `alsa-lib` (Arch)

## Development

```bash
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # production
```

## Tech Stack

- **Frontend**: TypeScript, Vite
- **Backend**: Rust, Tauri 2.0
- **Audio**: cpal (cross-platform), rustfft (spectral analysis)
- **Transcription**: whisper-rs (whisper.cpp bindings)
