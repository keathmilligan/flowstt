## Context
The transcription panel currently displays text by replacing the entire content each time new transcription data arrives. This creates a jarring user experience. The enhancement changes this to a streaming display where text is appended continuously, similar to live captioning.

## Goals / Non-Goals
- Goals:
  - Provide a smooth, streaming text display experience
  - Keep most recent transcription visible at all times
  - Use a readable fixed-width font bundled with the app
  - Visual polish with fade-out effect at top edge
- Non-Goals:
  - Full scrollable history (text beyond visible area is discarded)
  - Copy/paste functionality for transcription text
  - Text search or highlighting

## Decisions

### Font Selection: Fira Mono
- **Decision**: Bundle Fira Mono Regular as the transcription font
- **Rationale**: Fira Mono is a classic, highly readable monospace font designed by Mozilla. It's open source (SIL Open Font License), well-suited for continuous reading, and has excellent glyph coverage. Available from Google Fonts.
- **Alternatives considered**:
  - Source Code Pro: Also excellent, but Fira Mono has slightly better readability for prose
  - JetBrains Mono: More modern ligatures, but overkill for non-code text
  - System monospace: Would vary across platforms, inconsistent experience

### Text Color: Light Blue (#7DD3FC)
- **Decision**: Use `#7DD3FC` (Tailwind sky-300) for transcription text
- **Rationale**: This shade provides excellent contrast against the dark background (`#0a0f1a`), complements the existing blue accent colors in the UI (`#3b82f6`), and is easy on the eyes for continuous reading.
- **Alternatives considered**:
  - `#87CEEB` (sky blue): Slightly too pale, lower contrast
  - `#00BFFF` (deep sky blue): Too saturated, fatiguing for reading
  - `#93C5FD` (blue-300): Good but less distinctive from UI accent blue

### Fade Effect Implementation
- **Decision**: Use CSS mask-image with linear gradient for top fade
- **Rationale**: CSS masks are well-supported, performant, and don't require additional DOM elements. The fade creates a visual cue that content continues above without needing scroll indicators.
- **Implementation**: `mask-image: linear-gradient(to bottom, transparent 0%, black 15%)`

### Text Buffer Strategy
- **Decision**: Use line-count based truncation, keeping last N lines
- **Rationale**: Simple and predictable. When text exceeds the visible area, older lines at the top are removed. This keeps memory bounded and matches the "no history" requirement.
- **Buffer size**: Approximately 8-10 lines based on panel height and font size

### No Line Breaks Between Segments
- **Decision**: Append new transcription text with a single space separator
- **Rationale**: Creates continuous flowing text like live captioning. The STT engine already provides punctuated text, so no artificial breaks are needed.

## Risks / Trade-offs
- **Risk**: Font file adds ~100KB to bundle size
  - Mitigation: Fira Mono Regular subset (Latin characters only) is ~50KB
- **Risk**: Text truncation loses earlier content
  - Mitigation: This is intentional per requirements; no history needed
- **Risk**: Fade effect may not work in older browsers
  - Mitigation: Tauri uses modern Chromium; mask-image is well-supported

## Open Questions
- None; requirements are clear
