## Context
The application currently captures audio only from input devices (microphones) using cpal. Users want to transcribe and visualize audio playing from other applications on their desktop. On Linux, PipeWire provides direct access to monitor sources (sink outputs) and supports capturing from multiple sources simultaneously.

## Goals / Non-Goals
- Goals:
  - Enumerate and capture system audio on Linux via PipeWire monitor sources
  - Allow users to select between input devices, system audio, or mixed mode
  - Capture from multiple sources simultaneously and mix in software
  - Reuse existing audio processing pipeline (visualization, speech detection, transcription)
- Non-Goals:
  - Windows WASAPI loopback support (future enhancement)
  - macOS support (requires third-party virtual audio devices)
  - Per-application audio capture
  - Fallback to PulseAudio (PipeWire provides PulseAudio compatibility layer)

## Decisions

### Decision: Use PipeWire directly instead of cpal
Replace cpal with the `pipewire` crate for audio capture on Linux. PipeWire provides:
- Direct access to monitor sources (system audio) without workarounds
- Ability to capture from multiple sources simultaneously
- Consistent API for both input devices and sink monitors
- Better integration with modern Linux audio stack

**Alternatives considered:**
- cpal with ALSA backend: Cannot see PipeWire/PulseAudio monitor sources
- libpulse bindings: User rejected PulseAudio dependency; PipeWire is the modern standard
- Virtual loopback devices: Requires user configuration, not seamless

### Decision: Unified audio source model with source type enum
Keep the existing `AudioSourceType` enum (`Input`, `System`, `Mixed`) and modify the backend to use PipeWire for all audio capture.

### Decision: Software mixing for combined mode
When mixed mode is selected, create two PipeWire streams (input + system monitor) and sum samples in software. Apply 0.5 gain to each to prevent clipping.

### Decision: Dedicated PipeWire thread with channel communication
PipeWire objects are not thread-safe and must run on a dedicated thread with PipeWire's main loop. Use `pipewire::channel` or standard Rust channels to communicate between the PipeWire thread and Tauri's async runtime.

## Architecture

### PipeWire Thread Model
```
┌─────────────────────────────────────────────────────────────┐
│                      Tauri Main Thread                       │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────┐  │
│  │ Commands    │───▶│ Channel TX  │───▶│ Audio Processor │  │
│  │ (start/stop)│    │ (control)   │    │ (viz/speech)    │  │
│  └─────────────┘    └─────────────┘    └─────────────────┘  │
│                            │                   ▲             │
│                            ▼                   │             │
│                     ┌─────────────┐    ┌───────┴───────┐    │
│                     │ Channel RX  │    │ Channel TX    │    │
│                     │ (control)   │    │ (audio data)  │    │
└─────────────────────┼─────────────┼────┼───────────────┼────┘
                      │             │    │               │
┌─────────────────────┼─────────────┼────┼───────────────┼────┐
│                     ▼             │    ▲               │    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              PipeWire Main Loop Thread              │   │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────────┐    │   │
│  │  │ Registry │  │ Stream 1 │  │ Stream 2       │    │   │
│  │  │ (devices)│  │ (input)  │  │ (system audio) │    │   │
│  │  └──────────┘  └──────────┘  └────────────────┘    │   │
│  └─────────────────────────────────────────────────────┘   │
│                    PipeWire Thread                          │
└─────────────────────────────────────────────────────────────┘
```

### Stream Architecture
```
[Input Device Stream] ──┐
                        ├──▶ [Software Mixer] ──▶ [Audio Buffer] ──▶ [Processor Pipeline]
[System Audio Stream] ──┘         (if mixed)
```

For non-mixed modes, only one stream is active.

### Device Enumeration
PipeWire exposes devices via the Registry API:
- Input devices: nodes with `media.class = "Audio/Source"`
- System audio: nodes with `media.class = "Audio/Sink"` (connect to monitor port)

To capture system audio, use the `STREAM_CAPTURE_SINK` property which automatically connects to the sink's monitor.

## Implementation Notes

### PipeWire Stream Setup
```rust
// For input device capture
let props = properties! {
    *pipewire::keys::MEDIA_TYPE => "Audio",
    *pipewire::keys::MEDIA_CATEGORY => "Capture",
    *pipewire::keys::MEDIA_ROLE => "Communication",
};
stream.connect(Direction::Input, Some(node_id), flags, &mut [])?;

// For system audio (sink monitor) capture
let props = properties! {
    *pipewire::keys::MEDIA_TYPE => "Audio",
    *pipewire::keys::MEDIA_CATEGORY => "Capture",
    *pipewire::keys::STREAM_CAPTURE_SINK => "true",
};
stream.connect(Direction::Input, None, flags, &mut [])?;  // Auto-connects to default sink
```

### Audio Format
Request F32LE format at the device's native sample rate, then resample to 16kHz for Whisper:
```rust
let mut audio_info = AudioInfoRaw::new();
audio_info.set_format(AudioFormat::F32LE);
audio_info.set_rate(48000);  // Or negotiate with device
audio_info.set_channels(2);   // Stereo, convert to mono in processing
```

### Mixed Mode Synchronization
For initial implementation, accept that streams may have slight timing differences:
- Buffer samples from each stream
- Mix when both buffers have data
- Use minimum available samples to avoid drift accumulation

Future enhancement: Use PipeWire's graph timing for precise synchronization.

## Risks / Trade-offs
- **PipeWire required**: Users without PipeWire won't have audio capture. Mitigation: PipeWire is standard on modern Linux distros (Fedora, Ubuntu 22.10+, etc.)
- **Build dependency**: Requires `libpipewire-0.3-dev` for compilation. Mitigation: Document in README, consider static linking.
- **Thread complexity**: PipeWire's single-threaded model requires careful channel design. Mitigation: Clear separation of concerns, well-defined message protocol.
- **Latency in mixed mode**: Two streams may have slight timing differences. Mitigation: Accept small drift; users can see combined visualization.

## Open Questions
- Should we expose per-source volume controls in mixed mode? (Defer to future enhancement)
- Should we persist the last selected source type? (Yes, via frontend state)
- Should we provide fallback to cpal for non-PipeWire systems? (Defer - focus on PipeWire first)
