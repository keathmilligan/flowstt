<picture>
  <source srcset="assets/flowstt-landscape.svg" media="(prefers-color-scheme: dark)">
  <source srcset="assets/flowstt-landscape-light.svg" media="(prefers-color-scheme: light)">
  <img src="assets/flowstt-landscape.svg" alt="FlowSTT logo">
</picture>

A voice transcription agent for fluid, natural conversation. FlowSTT goes beyond simple speech-to-text with real-time cadence analysis, intelligent speech detection, and rich automation capabilities.

https://github.com/user-attachments/assets/a55a42d3-710c-4bb3-a4c1-539dd1696e5a

## Vision

Traditional voice assistants feel robotic. FlowSTT treats conversation as a continuous, adaptive stream where timing, cadence, and intent all matter. The system knows *when* to respond, not just *what* to respond to.

## Current Features

- **Audio Capture**: Microphone, system audio (loopback), or mixed mode
- **Echo Cancellation**: WebRTC AEC3-based echo removal for mixed mode capture
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
- [x] Echo cancellation (WebRTC AEC3 for mixed mode)
- [ ] Real-time cadence analysis (natural pause vs end-of-thought detection)
- [ ] Adaptive timeout management (context-aware listening windows)
- [ ] Acknowledgment feedback loop (accept tone, processing indicator)
- [ ] Interrupt handling (soft/hard interrupts with recovery)
- [ ] Dynamic query & follow-up behavior (clarifying questions)
- [ ] Multi-modal input (voice + CLI + gestures)
- [ ] Workflow automation (action execution from voice commands)

## Getting Started

Download the latest release from the releases page.

## Prerequisites

### Whisper Model

Download a model from [whisper.cpp models](https://huggingface.co/ggerganov/whisper.cpp/tree/main) and place it at:
- **Linux**: `~/.cache/whisper/ggml-base.en.bin`
- **macOS**: `~/Library/Caches/whisper/ggml-base.en.bin`
- **Windows**: `C:\Users\<username>\AppData\Local\whisper\ggml-base.en.bin`

### Build Dependencies

- Rust, Node.js, pnpm, CMake, C/C++ compiler
- **Linux**: `libasound2-dev` (Debian/Ubuntu) or `alsa-lib` (Arch)

### CUDA Acceleration (Linux & Windows)

For GPU-accelerated transcription on NVIDIA GPUs, you can enable CUDA support:

**Requirements:**
- NVIDIA GPU with CUDA support
- NVIDIA drivers with CUDA support (minimum driver version 525+ for CUDA 12.x)
- **Linux only**: NVIDIA CUDA Toolkit (nvcc, cuBLAS) - typically version 11.x or 12.x

**Build with CUDA:**
```bash
pnpm tauri build --features cuda
# or for development:
pnpm tauri dev --features cuda
```

**Platform differences:**

| Platform | CUDA Support | Build Requirements | Binary Size |
|----------|--------------|-------------------|-------------|
| Windows x64 | Yes | None (uses prebuilt binaries) | ~457MB additional |
| Linux | Yes | CUDA Toolkit required | ~20-50MB additional |
| macOS | No effect | N/A (uses Metal via prebuilt framework) | - |

**Windows notes:**
- CUDA binaries are downloaded automatically during build (~457MB)
- All required CUDA runtime DLLs are bundled with the application
- End users do not need to install the CUDA Toolkit, only NVIDIA drivers

**Linux troubleshooting:**
- Ensure `nvcc` is in your PATH: `nvcc --version`
- Install CUDA Toolkit: `sudo apt install nvidia-cuda-toolkit` (Debian/Ubuntu) or `sudo pacman -S cuda` (Arch)
- If build fails with cuBLAS errors, ensure `libcublas` is installed

## Development

```bash
pnpm install
pnpm tauri dev      # development (CPU-only)
pnpm tauri build    # production (CPU-only)
```

## Tech Stack

- **Frontend**: TypeScript, Vite
- **Backend**: Rust, Tauri 2.0
- **Audio**: PipeWire (Linux), rustfft (spectral analysis), aec3 (echo cancellation)
- **Transcription**: whisper-rs (whisper.cpp bindings)
