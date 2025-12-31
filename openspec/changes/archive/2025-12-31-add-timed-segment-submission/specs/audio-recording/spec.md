## MODIFIED Requirements

### Requirement: Automatic Transcription Mode
The system SHALL provide an automatic transcription mode where audio is captured continuously and speech segments are extracted for transcription based on speech detection events. When enabled, the system monitors for speech activity and extracts each speech segment for transcription without manual intervention. Segments SHALL be submitted for transcription after a maximum duration of 1 second, breaking at the next detected word boundary.

#### Scenario: Transcribe mode enabled
- **WHEN** the user enables the Transcribe toggle
- **THEN** the system begins continuous audio capture, monitoring for speech activity

#### Scenario: Continuous capture while transcribe active
- **WHEN** transcribe mode is active
- **THEN** audio samples are continuously written to a ring buffer regardless of speech state

#### Scenario: Speech triggers segment marking
- **WHEN** transcribe mode is active and the speech detector emits a speech-started event
- **THEN** the system marks the segment start position (including lookback samples) without interrupting capture

#### Scenario: Speech end triggers segment extraction
- **WHEN** transcribe mode is active and the speech detector emits a speech-ended event
- **THEN** the system extracts (copies) the segment from the ring buffer, saves it to a WAV file, and queues it for transcription

#### Scenario: Capture continues after segment extraction
- **WHEN** a speech segment is extracted from the ring buffer
- **THEN** audio capture continues uninterrupted, ready to capture the next segment

#### Scenario: Transcribe mode disabled
- **WHEN** the user disables the Transcribe toggle
- **THEN** the system stops audio capture and any in-progress segment is finalized and queued

#### Scenario: Segment submitted at word break after duration threshold
- **WHEN** transcribe mode is active and a speech segment exceeds 1 second duration
- **THEN** the system waits for the next word break event and extracts the segment up to that word boundary

#### Scenario: Segment submitted after grace period if no word break
- **WHEN** a speech segment exceeds 1 second duration and no word break is detected within 500ms
- **THEN** the system extracts the segment at the current position regardless of word boundary

#### Scenario: Speech state continues after timed segment
- **WHEN** a segment is extracted due to duration threshold (not speech end)
- **THEN** the system remains in speech state and begins a new segment from the word break position

## ADDED Requirements

### Requirement: Timed Segment Submission
The system SHALL limit speech segment duration to a maximum of 1 second during automatic transcription mode. When the duration threshold is reached, the segment is submitted at the next detected word break to produce incremental transcription results.

#### Scenario: Short utterance submitted on speech end
- **WHEN** speech lasts less than 1 second before ending
- **THEN** the segment is submitted when speech-ended is detected (existing behavior)

#### Scenario: Long utterance submitted at word break
- **WHEN** speech continues beyond 1 second
- **THEN** the segment is submitted at the next word break after the 1-second threshold

#### Scenario: Segment duration tracked from speech start
- **WHEN** a speech-started event occurs
- **THEN** the segment duration counter begins at zero (not counting lookback samples)

#### Scenario: Duration counter resets after timed submission
- **WHEN** a segment is submitted due to duration threshold
- **THEN** the duration counter resets to zero for the new segment

#### Scenario: Multiple timed segments from single utterance
- **WHEN** continuous speech lasts several seconds
- **THEN** multiple segments are extracted and queued at approximately 1-second intervals aligned to word breaks
