<picture>
  <source srcset="images/flowstt-landscape.svg" media="(prefers-color-scheme: dark)">
  <source srcset="images/flowstt-landscape-light.svg" media="(prefers-color-scheme: light)">
  <img src="images/flowstt-landscape.svg" alt="FlowSTT logo">
</picture>

---

FlowSTT is a free, privacy-first speech-to-text application that runs entirely on your local machine. No subscriptions, no signups, no cloud services —- your voice data never leaves your computer.

![FlowSTT Screenshot](images/flowstt-screenshot.png)

## Features

- **Local transcription** — Offline Whisper inference via whisper.cpp
- **Hardware accelerated** — CUDA on Windows/Linux, Metal on macOS
- **Real-time visualization** — Waveform, spectrogram, and speech activity graphs
- **Multi-source audio** — Microphone, system audio, or mixed mode with echo cancellation (WebRTC AEC3)
- **Cross-platform** — Windows (WASAPI), macOS (CoreAudio), Linux (PipeWire)
- **Scriptable CLI** — Full command-line interface with JSON output

## Installation

Download the latest release from the [Releases page](https://github.com/keath/flowstt/releases).

### Windows

1. Download `FlowSTT_X.X.X_x64-setup.exe` or `FlowSTT_X.X.X_x64.msi`
2. Run the installer
3. If Windows SmartScreen shows a warning, click **"More info"** then **"Run anyway"**
4. Launch FlowSTT from the Start Menu or Desktop shortcut

### macOS

1. Download `FlowSTT_aarch64.dmg` (Apple Silicon) or `FlowSTT_x64.dmg` (Intel)
2. Open the DMG file
3. Drag FlowSTT to your Applications folder
4. If macOS Gatekeeper prevents opening:
   - Right-click (or Control-click) on FlowSTT in Applications
   - Select **"Open"** from the context menu
   - Click **"Open"** in the dialog to confirm

### Linux

**DEB (Debian/Ubuntu):**
```bash
sudo dpkg -i flowstt_X.X.X_amd64.deb
sudo apt-get install -f  # Install dependencies
```

**RPM (Fedora/RHEL):**
```bash
sudo rpm -i flowstt-X.X.X.x86_64.rpm
```

**AppImage (Universal):**
```bash
chmod +x FlowSTT_X.X.X_x86_64.AppImage
./FlowSTT_X.X.X_x86_64.AppImage
```

**Note:** AppImage does not support auto-update. Download new versions manually from the Releases page.

## CLI Usage

```bash
flowstt list                    # List audio devices
flowstt transcribe              # Start transcription
flowstt status                  # Show service state
flowstt stop                    # Stop transcription
flowstt model                   # Show Whisper model status
flowstt model download          # Download Whisper model
flowstt gpu                     # Show GPU/CUDA status
flowstt config show             # Display configuration
flowstt config set key val      # Set config value
flowstt setup                   # Interactive first-time setup
```

Global flags: `--format json` for machine output, `-q/--quiet`, `-v/--verbose`.

## Development

### Architecture

FlowSTT runs as two binaries:

| Binary | Role |
|--------|------|
| `flowstt-app` | Tauri 2.0 desktop application with integrated audio engine, transcription, and IPC server |
| `flowstt` | CLI for headless operation and scripting |

The CLI communicates with the app over platform-native IPC (Unix sockets on Linux/macOS, named pipes on Windows). The app can be started in headless mode (`--headless`) for CLI-only usage.

### Build

Prerequisites: Rust toolchain, pnpm

```bash
# Install
pnpm install

# Standard build
make build

# Debug build
make build-debug

# Lint and test
make lint
make test

# Run the Service
make run-service

# Run the UI
pnpm tauri dev
```

## Tech Stack

- **Backend**: Rust, Tauri 2.0, whisper-rs, WebRTC AEC3, rustfft
- **Frontend**: TypeScript, Vite
- **Audio**: WASAPI (Windows), CoreAudio (macOS), PipeWire (Linux)

## License

MIT
