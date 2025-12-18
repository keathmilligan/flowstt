## 1. Font Bundling
- [ ] 1.1 Download Fira Mono Regular font file (WOFF2 format) from Google Fonts
- [ ] 1.2 Add font file to `src/assets/fonts/` directory
- [ ] 1.3 Add @font-face declaration in `src/styles.css` for Fira Mono

## 2. Transcription Panel Styling
- [ ] 2.1 Update `.result-box` styles with Fira Mono font-family
- [ ] 2.2 Change text color to light blue (#7DD3FC)
- [ ] 2.3 Add CSS mask-image gradient for top fade effect
- [ ] 2.4 Configure overflow behavior for text containment

## 3. Streaming Text Logic
- [ ] 3.1 Modify transcription-complete event handler to append text instead of replace
- [ ] 3.2 Add space separator between appended segments (no line breaks)
- [ ] 3.3 Implement auto-scroll to bottom after text append
- [ ] 3.4 Implement line-based buffer truncation to remove oldest content

## 4. Validation
- [ ] 4.1 Verify font renders correctly on Windows
- [ ] 4.2 Verify fade effect displays properly
- [ ] 4.3 Verify text appends continuously without line breaks
- [ ] 4.4 Verify auto-scroll keeps newest text visible
- [ ] 4.5 Verify old text is discarded when buffer limit reached
