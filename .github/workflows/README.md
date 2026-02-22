# GitHub Actions Workflows

FlowSTT uses GitHub Actions for CI/CD. Workflows run automatically on push to `master` and pull requests.

## Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push to `master`, PRs | Lint, type-check, and test |
| `release.yml` | Draft release created | Build and publish releases |

## Local CI Testing with `act`

You can run GitHub Actions workflows locally using [act](https://github.com/nektos/act):

```bash
# Install act (macOS)
brew install act

# Install act (Linux)
curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash

# Run with a specific platform (Linux only - requires Docker)
act push -j lint-and-test --matrix platform:ubuntu-22.04
```

> **Note:** Local CI testing requires Docker. Some platform-specific steps (macOS, Windows) cannot be tested locally with `act`.

## CUDA Builds

CUDA-accelerated builds are **not** run in CI because GitHub-hosted runners lack NVIDIA GPUs. The `cuda` feature is excluded from clippy and test runs:

```yaml
# src-service is linted/tested without --all-features
cargo clippy --all-targets -- -D warnings  # No cuda feature
cargo test                                   # No cuda feature
```

To test CUDA builds locally:

```bash
# Build with CUDA (requires NVIDIA CUDA Toolkit on Linux)
make build-cuda

# Or directly:
cargo build -p flowstt-service --release --features cuda
```

See the main [README.md](../../README.md#cuda-acceleration-linux--windows) for full CUDA requirements.

## Platform Matrix

### CI (`ci.yml`)

| Platform | Runner | Notes |
|----------|--------|-------|
| Linux | `ubuntu-22.04` | PipeWire PPA for audio dependencies |
| macOS | `macos-latest` | - |
| Windows | `windows-latest` | - |

### Release (`release.yml`)

| Platform | Runner | Target | Notes |
|----------|--------|--------|-------|
| Linux | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | Creates tar.gz archive |
| macOS | `macos-latest` | `aarch64-apple-darwin` | Apple Silicon |
| macOS | `macos-latest` | `x86_64-apple-darwin` | Intel |
| Windows | `windows-latest` | `x86_64-pc-windows-msvc` | - |

## Linux Dependencies

The following packages are installed on Ubuntu runners:

```bash
# PipeWire PPA (Ubuntu 22.04 ships 0.3.48, we need 0.3.65+)
sudo add-apt-repository -y ppa:pipewire-debian/pipewire-upstream

# Build dependencies
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \   # Tauri WebView
  libappindicator3-dev \    # System tray
  librsvg2-dev \            # SVG rendering
  patchelf \                # Binary patching
  libpipewire-0.3-dev \     # Audio capture
  libspa-0.2-dev \          # PipeWire SPA
  libclang-dev \            # FFI bindings
  cmake                     # whisper.cpp build
```
