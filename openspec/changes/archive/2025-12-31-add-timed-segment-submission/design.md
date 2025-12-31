# Design: Timed Segment Submission with Word Break Detection

## Context

The current automatic transcription mode waits for speech to end (via the 300ms hold time) before extracting and submitting a segment for transcription. For long continuous speech, this delays transcription significantly. Additionally, very long segments may be less efficient for Whisper to process.

The word break detection feature already identifies brief energy dips between words during active speech. This design leverages that existing capability to create more natural segment boundaries.

## Goals

- Submit segments for transcription after a maximum of 1 second of audio
- Break segments at natural word boundaries to improve transcription accuracy
- Maintain continuous speech detection state across segment boundaries
- Produce incremental transcription results during long speech

## Non-Goals

- Changing the speech detection algorithm
- Modifying Whisper transcription behavior
- Real-time streaming transcription (segments are still batched, just smaller)

## Decisions

### Decision: Maximum segment duration of 1 second

**Rationale:** 1 second provides a good balance between incremental results and sufficient context for Whisper. Shorter segments might lack context; longer segments delay results unnecessarily.

**Alternatives considered:**
- Configurable duration: Adds complexity without clear benefit. 1 second is a reasonable fixed default.
- 500ms segments: Too short, may produce worse transcription quality due to insufficient context.
- 2 second segments: Unnecessarily long for incremental transcription goals.

### Decision: Break at next word boundary after duration threshold

**Rationale:** Breaking mid-word would produce transcription artifacts. Waiting for a word break ensures clean segment boundaries.

Algorithm:
1. Track segment duration in samples
2. When duration exceeds 1 second (48,000 samples at 48kHz capture rate), set "seeking word break" flag
3. When word break is detected while flag is set, extract and submit segment at the word break position
4. Reset duration counter and continue in speech state with new segment

### Decision: Grace period for word break after threshold

**Rationale:** If no word break occurs (e.g., very fast speech or a sustained sound), we need a fallback to prevent unbounded segment growth.

- Grace period: 500ms after the 1-second threshold
- If no word break detected within grace period, submit segment immediately
- This caps maximum segment duration at ~1.5 seconds

**Alternatives considered:**
- No grace period (always wait for word break): Could result in very long segments if word breaks are sparse.
- Immediate submission on threshold: Would break mid-word, producing transcription artifacts.

### Decision: Segment boundary at word break midpoint

**Rationale:** Word breaks are detected as gaps between words. The word break event provides an offset from speech start, which can be used to calculate the extraction point. Using the midpoint of the gap ensures we capture the complete previous word and leave room for the start of the next word.

### Decision: Continue speech state across timed segments

**Rationale:** When a segment is submitted due to the duration threshold (not speech end), we remain in speech state. The next segment begins immediately from the word break position, preserving any lookback that might be needed.

```
Speech Timeline:
─────────────────────────────────────────────────────────────▶
│           │          │           │          │              │
speech-   word-     word-       word-      word-         speech-
started   break     break       break      break          ended
│           │          │           │          │              │
└───────────┼──────────┘           │          │              │
   Segment 1 (1s)       └──────────┼──────────┘              │
                       Segment 2 (1s)         └──────────────┘
                                             Segment 3 (final)
```

### Decision: Reuse existing word break events

**Rationale:** The `word-break` event already provides the necessary information (offset_ms, gap_duration_ms). The TranscribeState can listen for these events to determine when to submit segments.

Implementation approach:
- Add an `on_word_break` method to TranscribeState
- Track whether we're seeking a word break (duration threshold exceeded)
- When word break is received while seeking, extract segment up to the word break position

## Data Flow

```
Audio Callback Loop:
┌─────────────────────────────────────────────────────────────┐
│  1. Samples arrive                                           │
│  2. Write to ring buffer                                     │
│  3. Update segment duration counter                          │
│  4. If duration > 1s: set seeking_word_break = true          │
│  5. Process through speech detector                          │
│     - May emit word-break event                              │
│  6. If seeking_word_break && word-break received:            │
│     - Extract segment up to word break                       │
│     - Queue for transcription                                │
│     - Reset segment start to word break position             │
│     - Clear seeking_word_break                               │
│  7. If seeking_word_break && grace period exceeded:          │
│     - Extract segment at current position                    │
│     - Queue for transcription                                │
│     - Reset segment start                                    │
│     - Clear seeking_word_break                               │
└─────────────────────────────────────────────────────────────┘
```

## Risks / Trade-offs

- **Transcription at word boundary vs mid-word:** Breaking at word boundaries should produce better transcription than mid-word breaks. The 500ms grace period ensures we don't wait indefinitely.

- **Word break detection accuracy:** If word breaks are not detected accurately, segments may not break at optimal points. However, the grace period fallback ensures segments are still submitted.

- **Increased number of segments:** More segments means more transcription queue activity and more WAV files. This is acceptable given the bounded queue size (10 segments) and the benefit of incremental results.

- **Context across segment boundaries:** Whisper may have less context for the start of a new segment. However, 1 second of audio typically provides sufficient context for accurate transcription.

## Open Questions

- Should the maximum segment duration be configurable? (Initial decision: No, keep it simple with a fixed 1-second duration)
