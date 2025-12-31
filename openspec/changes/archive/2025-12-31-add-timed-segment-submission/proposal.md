# Change: Timed Segment Submission with Word Break Detection

## Why

Currently, speech segments are submitted for transcription only when speech ends (detected via the hold time threshold). For long continuous speech, this can result in significant delays before transcription begins, and very long segments may be less efficient for Whisper to process. By limiting segment duration to a maximum of 1 second and breaking at natural word boundaries, transcription can begin sooner and produce more incremental results.

## What Changes

- Add a maximum segment duration (1 second) for automatic transcription mode
- When the maximum duration is reached during active speech, submit the segment at the next detected word break rather than immediately
- If no word break occurs within a grace period after the maximum duration, submit the segment at that point regardless
- Reuse the existing word break detection feature to identify natural break points

## Impact

- Affected specs: `audio-recording`, `speech-transcription`
- Affected code: `src-tauri/src/transcribe_mode.rs` (TranscribeState), `src-tauri/src/audio.rs` (audio callback coordination)
- Dependencies: Relies on existing word break detection in `processor.rs`
