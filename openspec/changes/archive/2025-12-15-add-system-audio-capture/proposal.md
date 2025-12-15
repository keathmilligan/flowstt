# Change: Add System Audio Capture Support

## Why
Users need to transcribe audio from other applications (meetings, videos, music) and visualize system audio output. Currently, the app only captures microphone input, limiting its usefulness for transcribing content playing on the desktop.

## What Changes
- Replace cpal with PipeWire for audio capture on Linux
- Add system audio (loopback) device enumeration via PipeWire Registry
- Support monitoring and recording from system audio sources (sink monitors)
- Allow mixing of system audio with microphone input in software
- Run PipeWire on dedicated thread with channel-based communication to Tauri

## Impact
- Affected specs: `audio-recording`
- Affected code: `src-tauri/src/audio.rs`, `src-tauri/src/lib.rs`, `src/main.ts`
- New file: `src-tauri/src/pipewire_audio.rs`
- New dependency: `pipewire = "0.9"`
- Build requirement: `libpipewire-0.3-dev` (Linux)
- UI element: Audio source type selector (Input / System / Mixed) - already implemented

## Technical Approach
PipeWire provides direct access to:
- Input devices via `media.class = "Audio/Source"` nodes
- System audio via sink monitor capture (`STREAM_CAPTURE_SINK` property)
- Multiple simultaneous streams for mixed mode

The implementation uses a dedicated PipeWire thread running the PipeWire main loop, with Rust channels for communication with Tauri's async runtime.
