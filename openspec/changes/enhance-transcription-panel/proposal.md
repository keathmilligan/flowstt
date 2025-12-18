# Change: Enhance Voice Transcription Panel

## Why
The current transcription panel displays text statically and replaces content entirely on each update. Users need a streaming display that shows continuously updating text output, with new text appending to the end and automatic scrolling to keep the most recent content visible.

## What Changes
- Replace static text display with streaming append-only text behavior
- Add automatic scroll-to-bottom so newest text is always visible
- Remove line breaks between transcription segments (continuous flow)
- Change text color to light blue that complements the dark UI theme
- Use Fira Mono, a classic readable fixed-width font bundled with the application
- Add fade-out gradient effect at the top of the panel to indicate more content above
- Limit displayed content to last few lines (no scrollable history)

## Impact
- Affected specs: audio-visualization (transcription display is part of the visualization UI)
- Affected code:
  - `src/styles.css` - Font face, text color, fade effect styling
  - `src/main.ts` - Text append logic, auto-scroll behavior
  - `index.html` - Font loading (if using @font-face)
  - `src/assets/` - Bundled Fira Mono font files
