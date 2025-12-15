# FlowSTT

A desktop application for speech-to-text transcription using local Whisper inference.

## Features

- Enumerate and select audio input devices
- Record audio from selected device
- Transcribe speech to text using Whisper (offline, local processing)

## Prerequisites

### Whisper Model

Before using the app, you need to download a Whisper model file:

1. Visit: https://huggingface.co/ggerganov/whisper.cpp/tree/main
2. Download `ggml-base.en.bin` (145 MB) - or choose another model size
3. Place it at:
   - **Linux**: `~/.cache/whisper/ggml-base.en.bin`
   - **macOS**: `~/Library/Caches/whisper/ggml-base.en.bin`
   - **Windows**: `C:\Users\<username>\AppData\Local\whisper\ggml-base.en.bin`

Available models (larger = more accurate but slower):
- `ggml-tiny.en.bin` - 75 MB
- `ggml-base.en.bin` - 145 MB (recommended)
- `ggml-small.en.bin` - 465 MB
- `ggml-medium.en.bin` - 1.5 GB

### Build Dependencies

- Rust toolchain
- Node.js and pnpm
- CMake (required to build whisper.cpp)
- C/C++ compiler (gcc/clang)
- Platform audio libraries:
  - **Linux**: `alsa-lib` development headers (e.g., `libasound2-dev` on Debian/Ubuntu, `alsa-lib` on Arch)
  - **macOS**: CoreAudio (included with Xcode)
  - **Windows**: WASAPI (included with Windows SDK)

## Development

```bash
# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Usage

1. Launch the application
2. Select an audio input device from the dropdown
3. Click "Record" to start recording
4. Speak into the microphone
5. Click "Stop" to stop recording and begin transcription
6. View the transcribed text in the result area

## Tech Stack

- **Frontend**: TypeScript, Vite
- **Backend**: Rust, Tauri 2.0
- **Audio**: cpal (cross-platform audio I/O)
- **Transcription**: whisper-rs (whisper.cpp bindings)

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
