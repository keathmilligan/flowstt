# Tasks: Timed Segment Submission with Word Break Detection

## 1. Update TranscribeState for Timed Segments

- [x] 1.1 Add segment duration tracking (sample count from segment start)
- [x] 1.2 Add `seeking_word_break` flag to indicate duration threshold exceeded
- [x] 1.3 Add `word_break_grace_start` timestamp for grace period tracking
- [x] 1.4 Define constants: MAX_SEGMENT_DURATION_MS (1000), WORD_BREAK_GRACE_MS (500)

## 2. Implement Word Break Handling in TranscribeState

- [x] 2.1 Add `on_word_break` method to handle word-break events
- [x] 2.2 Calculate extraction position from word break offset
- [x] 2.3 Extract segment up to word break position when seeking
- [x] 2.4 Reset segment start to word break position after extraction
- [x] 2.5 Queue extracted segment for transcription

## 3. Implement Duration Threshold Logic

- [x] 3.1 Track segment duration in `process_samples` method
- [x] 3.2 Set `seeking_word_break` flag when duration exceeds threshold
- [x] 3.3 Record grace period start time when threshold exceeded

## 4. Implement Grace Period Fallback

- [x] 4.1 Check grace period in `process_samples` when seeking word break
- [x] 4.2 Force segment extraction if grace period exceeded without word break
- [x] 4.3 Reset state after grace period extraction

## 5. Integrate with Audio Callback

- [x] 5.1 Forward word-break events from speech detector to TranscribeState
- [x] 5.2 Ensure segment duration is updated before speech detection
- [x] 5.3 Verify event ordering (duration check -> speech detection -> word break handling)

## 6. Testing

- [x] 6.1 Verify segments are submitted after ~1 second during continuous speech
- [x] 6.2 Verify segments break at word boundaries when available
- [x] 6.3 Verify grace period fallback works when no word break detected
- [x] 6.4 Verify speech state continues across segment boundaries
- [x] 6.5 Verify normal speech-ended behavior still works for shorter utterances
