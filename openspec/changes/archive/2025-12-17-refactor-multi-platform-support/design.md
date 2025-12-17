## Context

FlowSTT currently only supports Linux using PipeWire for audio capture. The application uses PipeWire-specific types and APIs throughout `lib.rs` and `pipewire_audio.rs`. To support Windows and macOS, the audio capture layer needs abstraction while preserving the existing processing pipeline (visualization, speech detection, transcription).

### Stakeholders
- End users on Windows, macOS, and Linux
- Developers implementing platform-specific audio backends

### Constraints
- Must maintain full Linux functionality (no regression)
- Windows and macOS implementations are stubs in this phase
- Frontend is already platform-agnostic (TypeScript + Tauri invoke)
- Audio processing pipeline (`processor.rs`, `audio.rs`, `transcribe.rs`) is platform-independent

## Goals / Non-Goals

### Goals
- Define a clean `AudioBackend` trait for platform-agnostic audio capture
- Reorganize code into platform-specific modules (`platform/linux/`, `platform/windows/`, `platform/macos/`)
- Use Rust conditional compilation for automatic platform selection
- Maintain backward compatibility on Linux
- Provide clear stub implementations that indicate "not implemented" status

### Non-Goals
- Full Windows audio implementation (future change)
- Full macOS audio implementation (future change)
- Changes to the frontend
- Changes to audio processing, visualization, or transcription

## Decisions

### Decision: Use a trait-based abstraction
The `AudioBackend` trait will define the interface for audio capture operations. Platform implementations will implement this trait. The main application code will work with `Box<dyn AudioBackend>` to enable runtime polymorphism.

**Rationale**: Trait-based abstraction is idiomatic Rust and allows clean separation of platform-specific code. It enables future platforms to be added without modifying core application logic.

**Alternative considered**: Feature flags per platform - rejected because trait abstraction is cleaner and allows potential runtime backend selection in the future.

### Decision: Module structure with `platform/` directory

```
src-tauri/src/
  platform/
    mod.rs           # Re-exports platform-specific backend via cfg
    linux/
      mod.rs         # Linux platform module
      pipewire.rs    # PipeWire implementation (moved from pipewire_audio.rs)
    windows/
      mod.rs         # Windows platform module
      stub.rs        # Stub implementation
    macos/
      mod.rs         # macOS platform module
      stub.rs        # Stub implementation
  audio.rs           # Unchanged - platform-independent types
  processor.rs       # Unchanged - audio processing
  transcribe.rs      # Unchanged - transcription
  lib.rs             # Updated to use platform::AudioBackend
```

**Rationale**: Clean separation by platform allows each implementation to grow independently. The `platform/mod.rs` uses conditional compilation to export the correct backend for the target platform.

### Decision: Keep pipewire-rs as Linux-only dependency
The `pipewire` crate will be conditionally included only on Linux builds using Cargo's target-specific dependencies.

```toml
[target.'cfg(target_os = "linux")'.dependencies]
pipewire = "0.8"
```

**Rationale**: Avoids compilation errors on Windows/macOS where PipeWire is not available.

### Decision: Stub implementations return descriptive errors
Stub backends on Windows and macOS will return `Err("Audio backend not implemented for this platform")` for all operations. This allows the application to compile and run, showing users a clear message about platform support status.

**Rationale**: Better user experience than compilation failure or cryptic runtime errors.

## AudioBackend Trait Design

```rust
/// Platform-agnostic audio backend trait
pub trait AudioBackend: Send + Sync {
    /// Create a new backend instance
    fn new(
        aec_enabled: Arc<Mutex<bool>>,
        recording_mode: Arc<Mutex<RecordingMode>>,
    ) -> Result<Self, String> where Self: Sized;

    /// List available input devices (microphones)
    fn list_input_devices(&self) -> Vec<AudioDevice>;

    /// List available system audio devices (monitors/loopback)
    fn list_system_devices(&self) -> Vec<AudioDevice>;

    /// Get current sample rate
    fn sample_rate(&self) -> u32;

    /// Start audio capture from specified sources
    fn start_capture_sources(
        &self,
        source1_id: Option<u32>,
        source2_id: Option<u32>,
    ) -> Result<(), String>;

    /// Stop audio capture
    fn stop_capture(&self) -> Result<(), String>;

    /// Try to receive audio samples (non-blocking)
    fn try_recv(&self) -> Option<AudioSamples>;
}

/// Audio samples from capture (platform-independent)
pub struct AudioSamples {
    pub samples: Vec<f32>,
    pub channels: u16,
}
```

## Risks / Trade-offs

### Risk: Platform-specific quirks in device IDs
Different platforms may use different ID formats (u32 on PipeWire, string UUIDs on Windows/macOS).

**Mitigation**: Define ID as `String` in the public interface, convert internally per platform.

### Risk: Different audio formats/sample rates per platform
Each platform may have different native formats.

**Mitigation**: The AudioBackend trait specifies a `sample_rate()` method. All implementations should normalize to stereo f32 format (existing Linux behavior). Format conversion is already handled in `audio.rs`.

### Risk: Conditional compilation complexity
Multiple `#[cfg(...)]` blocks can become difficult to maintain.

**Mitigation**: Isolate platform-specific code in the `platform/` directory. Use a single re-export point in `platform/mod.rs`.

## Migration Plan

1. Create `platform/` directory structure
2. Move `pipewire_audio.rs` to `platform/linux/pipewire.rs`
3. Implement `AudioBackend` trait in Linux module (wrapping existing PipeWireBackend)
4. Create stub implementations for Windows and macOS
5. Update `lib.rs` to use the platform abstraction
6. Update `Cargo.toml` for platform-specific dependencies
7. Test on Linux to verify no regression
8. Build on Windows/macOS to verify compilation (stubs return errors)

### Rollback
If issues are found, the original `pipewire_audio.rs` structure can be restored by reverting the changes.

## Open Questions

1. **Device ID type**: Should we use `String` everywhere for maximum flexibility, or keep `u32` and add conversions? (Recommendation: Use `String` for public interface)

2. **AEC implementation on other platforms**: The current AEC uses `aec3` crate. Is this portable? (Research needed for future implementation phases)
