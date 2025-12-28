# Design: Linux CUDA Acceleration

## Context

The current Linux implementation uses `whisper-rs` version 0.14, which wraps `whisper-rs-sys` to build whisper.cpp from source at compile time. The `whisper-rs` crate provides a `cuda` feature flag that, when enabled, configures the underlying whisper.cpp build to use NVIDIA CUDA for GPU-accelerated inference.

This is a Linux-only change because:
- Windows and macOS use prebuilt whisper.cpp binaries via FFI (not whisper-rs)
- Adding CUDA support to those platforms would require sourcing CUDA-enabled prebuilt binaries or changing the build approach entirely

## Goals

- Enable opt-in CUDA acceleration for Linux users with NVIDIA GPUs
- Maintain backward compatibility (CPU-only builds remain the default)
- Keep the implementation minimal by leveraging existing whisper-rs CUDA support

## Non-Goals

- CUDA support for Windows or macOS (different architecture, uses prebuilt binaries)
- ROCm/hipBLAS support for AMD GPUs (can be added separately if needed)
- Vulkan or OpenCL GPU backends
- Automatic GPU detection at runtime (feature is compile-time)

## Decisions

### Decision: Use Cargo feature flag propagation

**What**: Add a `cuda` feature to FlowSTT's Cargo.toml that enables `whisper-rs/cuda`.

**Why**:
- The `whisper-rs` crate already supports CUDA via its `cuda` feature flag
- Cargo's feature propagation mechanism is the standard way to expose optional dependencies
- No code changes required in the transcription module

**Alternatives considered**:
- Environment variable at build time: Less discoverable, non-standard
- Runtime GPU detection: Complex, requires shipping both CPU and GPU code

### Decision: Linux-only feature gate

**What**: The `cuda` feature only affects the Linux build (where whisper-rs is used).

**Why**:
- Windows/macOS use prebuilt binaries via FFI, not whisper-rs
- Adding CUDA to other platforms would require significant additional work
- Linux is the primary platform where users have direct access to CUDA toolchains

### Decision: Opt-in only, no auto-detection

**What**: Users must explicitly enable CUDA at build time with `--features cuda`.

**Why**:
- CUDA requires the NVIDIA CUDA Toolkit installed at build time
- Not all Linux users have NVIDIA GPUs
- Explicit opt-in avoids build failures for users without CUDA installed
- Clear documentation can guide users who want GPU acceleration

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| CUDA Toolkit version incompatibility | Document required CUDA version; whisper-rs-sys handles most compatibility |
| Build failures when CUDA not installed | Feature is opt-in; clear error message from whisper-rs-sys |
| Larger binary size with CUDA | Only affects builds with feature enabled |
| GPU memory requirements | Model size determines memory needs; document in user guide |

## Implementation Approach

### Cargo.toml Changes

```toml
[features]
default = []
cuda = ["whisper-rs/cuda"]

[target.'cfg(target_os = "linux")'.dependencies]
whisper-rs = { version = "0.14", optional = false }

# When cuda feature is enabled, whisper-rs is compiled with CUDA support
# The feature propagation happens automatically via the cuda = ["whisper-rs/cuda"] definition
```

Note: Since `whisper-rs` is already a non-optional dependency for Linux, the feature flag simply enables the CUDA sub-feature of that dependency.

### Build Commands

```bash
# Default CPU-only build
cargo build --release

# CUDA-enabled build (Linux only)
cargo build --release --features cuda

# Tauri build with CUDA
cargo tauri build --features cuda
```

## Open Questions

None - the implementation is straightforward since whisper-rs already provides CUDA support.
