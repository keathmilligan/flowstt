## 1. Font Bundling
- [x] 1.1 Download Fira Mono Regular font file (WOFF2 format) from Google Fonts
- [x] 1.2 Add font file to `src/assets/fonts/` directory
- [x] 1.3 Add @font-face declaration in `src/styles.css` for Fira Mono

## 2. Transcription Panel Styling
- [x] 2.1 Update `.result-box` styles with Fira Mono font-family
- [x] 2.2 Change text color to light blue (#7DD3FC)
- [x] 2.3 Add CSS mask-image gradient for top fade effect
- [x] 2.4 Configure overflow behavior for text containment

## 3. Streaming Text Logic
- [x] 3.1 Modify transcription-complete event handler to append text instead of replace
- [x] 3.2 Add space separator between appended segments (no line breaks)
- [x] 3.3 Implement auto-scroll to bottom after text append
- [x] 3.4 Implement line-based buffer truncation to remove oldest content

## 4. Validation
- [x] 4.1 Verify font renders correctly on Windows
- [x] 4.2 Verify fade effect displays properly
- [x] 4.3 Verify text appends continuously without line breaks
- [x] 4.4 Verify auto-scroll keeps newest text visible
- [x] 4.5 Verify old text is discarded when buffer limit reached
