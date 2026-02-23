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
- **Hardware accelerated** — CUDA on Windows, Metal on macOS
- **Real-time visualization** — Waveform, spectrogram, and speech activity graphs
- **Multi-source audio** — Microphone, system audio, or mixed mode with echo cancellation (WebRTC AEC3)
- **Cross-platform** — Windows (WASAPI) and macOS (CoreAudio)
- **Scriptable CLI** — Full command-line interface with JSON output

## Installation

Download the latest release from the [Releases page](https://github.com/flowstt/flowstt/releases).

<!-- release-links:start -->
**Current release:** v0.1.3

**Download packages**
- Windows: [FlowSTT_0.1.3_x64.msi](https://github.com/flowstt/flowstt/releases/download/v0.1.3/FlowSTT_0.1.3_x64.msi)
- macOS (Apple Silicon M-Series): [FlowSTT_aarch64.dmg](https://github.com/flowstt/flowstt/releases/download/v0.1.3/FlowSTT_aarch64.dmg)
- macOS (Legacy Intel x64): [FlowSTT_x64.dmg](https://github.com/flowstt/flowstt/releases/download/v0.1.3/FlowSTT_x64.dmg)
<!-- release-links:end -->

### Windows

1. Download `FlowSTT_0.1.3_x64.msi`
2. Run the installer — the MSI is code-signed, so no SmartScreen warnings
3. Launch FlowSTT from the Start Menu or Desktop shortcut

### macOS

1. Download `FlowSTT_aarch64.dmg` (Apple Silicon) or `FlowSTT_x64.dmg` (Intel)
2. Open the DMG file
3. Drag FlowSTT to your Applications folder
4. If macOS Gatekeeper prevents opening:
   - Right-click (or Control-click) on FlowSTT in Applications
   - Select **"Open"** from the context menu
   - Click **"Open"** in the dialog to confirm


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

## Windows Code Signing

Windows release builds are signed using [Azure Artifact Signing](https://learn.microsoft.com/en-us/azure/trusted-signing/overview) via [`trusted-signing-cli`](https://github.com/Levminer/trusted-signing-cli). Signing is automatic in CI when the required secrets are configured and is skipped otherwise (local builds are never signed).

### GitHub Repository Secrets

| Secret | Description |
|--------|-------------|
| `AZURE_TENANT_ID` | Microsoft Entra ID (Azure AD) tenant/directory ID |
| `AZURE_CLIENT_ID` | App Registration application (client) ID |
| `AZURE_CLIENT_SECRET` | App Registration client secret value |
| `AZURE_SIGNING_ENDPOINT` | Regional endpoint URL (e.g. `https://eus.codesigning.azure.net`) |
| `AZURE_SIGNING_ACCOUNT` | Artifact Signing account name |
| `AZURE_CERT_PROFILE` | Certificate profile name |

### Azure Infrastructure Setup

Before configuring the secrets above, complete these steps in the Azure Portal:

1. **Create an Azure subscription** (Pay-As-You-Go)
2. **Register the `Microsoft.CodeSigning` resource provider** in your subscription
3. **Create an Artifact Signing account** — note the account name and regional endpoint URL
4. **Create an identity validation** (Public Trust) — requires business name, address, tax ID, and a primary/secondary email on a domain you own. Validation may take hours to days.
5. **Create a certificate profile** (Public Trust) linked to the validated identity — note the profile name
6. **Create an App Registration** in Microsoft Entra ID — note the application (client) ID, directory (tenant) ID, and create a client secret
7. **Assign the `Artifact Signing Certificate Profile Signer` role** to the App Registration on the Artifact Signing account's IAM page

See the [Azure Artifact Signing quickstart](https://learn.microsoft.com/en-us/azure/trusted-signing/quickstart) for detailed instructions.

## Tech Stack

- **Backend**: Rust, Tauri 2.0, whisper-rs, WebRTC AEC3, rustfft
- **Frontend**: TypeScript, Vite
- **Audio**: WASAPI (Windows), CoreAudio (macOS)

## License

MIT
