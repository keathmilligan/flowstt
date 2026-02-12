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

### Command Line Interface

FlowSTT includes a powerful CLI for headless operation and scripting:

```bash
# List available audio devices
flowstt list

# Start transcription with default microphone
flowstt transcribe --source1 <device-id>

# Start transcription with two sources and echo cancellation
flowstt transcribe --source1 <mic-id> --source2 <system-id> --aec

# Check transcription status
flowstt status

# Stop transcription
flowstt stop

# Check GPU/CUDA status
flowstt gpu

# Download the Whisper model
flowstt model download

# Get JSON output for scripting
flowstt list --format json
flowstt status --format json
```

The CLI automatically starts the background service if not already running.

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
pnpm install        # install frontend dependencies
make build          # build all components (release)
make build-debug    # build all components (debug, faster compilation)
make build-cuda     # build with CUDA GPU acceleration
```

### Running Components

Each component can be built and run individually via `make` or `cargo`:

```bash
# Service (background daemon - must be running for CLI/GUI)
make run-service              # build + run (debug)
cargo run -p flowstt-service  # equivalent cargo command

# CLI
make run-cli                  # build + run (debug)
cargo run -p flowstt-cli      # equivalent cargo command

# GUI app
make run-app                  # build + run (debug)
cargo run -p flowstt-app      # equivalent cargo command
```

Release variants are available as `run-service-release`, `run-app-release`, `run-cli-release`.

### Quality Checks

```bash
make lint           # clippy + tsc --noEmit
make test           # cargo test --workspace
```

Run `make help` for all available targets.

### Architecture

FlowSTT consists of three main components:

| Component | Binary | Description |
|-----------|--------|-------------|
| **CLI** | `flowstt` | Command-line interface for scripting and headless operation |
| **Service** | `flowstt-service` | Background daemon handling audio capture and transcription |
| **GUI** | `flowstt-app` | Desktop application with visualization |

The CLI and GUI communicate with the service via IPC (Unix sockets on Linux/macOS, named pipes on Windows).

## Tech Stack

- **Frontend**: TypeScript, Vite
- **Backend**: Rust, Tauri 2.0
- **Audio**: PipeWire (Linux), rustfft (spectral analysis), aec3 (echo cancellation)
- **Transcription**: whisper-rs (whisper.cpp bindings)
