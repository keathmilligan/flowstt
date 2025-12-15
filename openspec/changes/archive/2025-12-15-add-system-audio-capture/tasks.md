## 1. Backend: PipeWire Integration Setup
- [x] 1.1 Add `pipewire` crate dependency to Cargo.toml
- [x] 1.2 Create `pipewire_audio.rs` module for PipeWire-specific code
- [x] 1.3 Implement PipeWire thread with main loop and channel communication
- [x] 1.4 Define message types for control (start/stop/enumerate) and data (audio samples)

## 2. Backend: Device Enumeration via PipeWire
- [x] 2.1 Implement Registry listener for device discovery
- [x] 2.2 Enumerate input devices (media.class = "Audio/Source")
- [x] 2.3 Enumerate system audio sinks (media.class = "Audio/Sink")
- [x] 2.4 Map PipeWire nodes to AudioDevice struct with friendly names
- [x] 2.5 Update Tauri commands to use PipeWire enumeration

## 3. Backend: Single Source Audio Capture
- [x] 3.1 Implement PipeWire Stream creation for input devices
- [x] 3.2 Implement PipeWire Stream creation for sink monitors (STREAM_CAPTURE_SINK)
- [x] 3.3 Handle audio format (F32LE, sample rate, channels)
- [x] 3.4 Forward audio samples to processing pipeline via channel
- [x] 3.5 Integrate with existing visualization and speech detection processors

## 4. Backend: Mixed Audio Capture
- [x] 4.1 Support creating two simultaneous streams (input + system)
- [ ] 4.2 Implement software mixer (sum samples with 0.5 gain each)
- [ ] 4.3 Handle buffer synchronization between streams
- [x] 4.4 Forward mixed audio to processing pipeline

## 5. Backend: Recording and Transcription
- [x] 5.1 Accumulate samples during recording (single or mixed mode)
- [x] 5.2 Convert to mono and resample to 16kHz for Whisper
- [x] 5.3 Integrate with existing transcription pipeline

## 6. Frontend: Source Type Selection UI
- [x] 6.1 Add source type selector (Input / System / Mixed) to UI
- [x] 6.2 Update device dropdown to show appropriate devices based on source type
- [x] 6.3 Update monitor/record commands to pass source type
- [x] 6.4 Persist selected source type in local state

## 7. Cleanup and Migration
- [ ] 7.1 Remove or gate cpal-based audio capture code
- [ ] 7.2 Remove unused mixer scaffolding from audio.rs
- [ ] 7.3 Update error handling for PipeWire-specific errors
- [ ] 7.4 Add build documentation for libpipewire-dev dependency

## 8. Validation
- [ ] 8.1 Test input-only mode with PipeWire backend
- [ ] 8.2 Test system audio monitoring and recording
- [ ] 8.3 Test mixed mode with simultaneous mic and system audio
- [ ] 8.4 Verify visualization works with all source types
- [ ] 8.5 Test transcription quality with system audio
