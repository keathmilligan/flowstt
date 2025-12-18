## ADDED Requirements

### Requirement: Streaming Transcription Display
The system SHALL display transcription output as a continuously streaming text panel that appends new text to the existing content rather than replacing it entirely.

#### Scenario: Text appends on new transcription
- **WHEN** new transcription text is received from the speech-to-text engine
- **THEN** the text is appended to the end of the existing displayed text with a space separator

#### Scenario: No line breaks between segments
- **WHEN** multiple transcription segments are received
- **THEN** they are joined as continuous flowing text without line breaks between them

#### Scenario: Auto-scroll to newest text
- **WHEN** new text is appended to the transcription display
- **THEN** the display automatically scrolls to show the most recent text at the bottom

### Requirement: Transcription Text Styling
The system SHALL display transcription text using a light blue color and fixed-width font for readability.

#### Scenario: Fixed-width font rendering
- **WHEN** transcription text is displayed
- **THEN** it renders using the bundled Fira Mono font

#### Scenario: Light blue text color
- **WHEN** transcription text is displayed
- **THEN** the text color is light blue (#7DD3FC) providing good contrast against the dark background

### Requirement: Transcription Panel Fade Effect
The system SHALL display a fade-out gradient at the top of the transcription panel to indicate content continues above the visible area.

#### Scenario: Top fade gradient visible
- **WHEN** the transcription panel contains text
- **THEN** a gradient fade effect is visible at the top edge, transitioning from transparent to fully visible over approximately 15% of the panel height

#### Scenario: Fade does not obscure recent text
- **WHEN** text is displayed in the panel
- **THEN** the most recent text at the bottom of the panel is fully visible without any fade effect

### Requirement: Transcription Buffer Limit
The system SHALL limit the transcription display to the most recent text that fits within the panel, discarding older content.

#### Scenario: Old text discarded when buffer full
- **WHEN** the accumulated transcription text exceeds the visible capacity of the panel
- **THEN** the oldest text is removed from the beginning to keep only the most recent content visible

#### Scenario: No scrollable history
- **WHEN** the user attempts to scroll the transcription panel
- **THEN** no additional history is available beyond what is currently visible

### Requirement: Bundled Transcription Font
The system SHALL bundle the Fira Mono font with the application for consistent transcription text rendering across platforms.

#### Scenario: Font loads from application bundle
- **WHEN** the application starts
- **THEN** the Fira Mono font is loaded from the bundled font files

#### Scenario: Font available offline
- **WHEN** the application runs without network connectivity
- **THEN** the transcription font renders correctly from the local bundle
