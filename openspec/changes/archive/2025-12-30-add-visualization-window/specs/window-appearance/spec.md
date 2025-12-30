## MODIFIED Requirements

### Requirement: Fixed Window Size
The main application window SHALL be non-resizable with a fixed compact size of 800x300 pixels.

#### Scenario: User attempts to resize main window
- **WHEN** the user attempts to resize the main window by dragging edges or corners
- **THEN** the window remains at its fixed 800x300 size

## ADDED Requirements

### Requirement: Visualization Window
The system SHALL provide a separate resizable window for displaying audio visualizations (waveform, spectrogram, speech activity graph).

#### Scenario: Visualization window opens on demand
- **WHEN** the user double-clicks the mini waveform in the main window header
- **THEN** the visualization window opens displaying all three visualizations

#### Scenario: Visualization window is resizable
- **WHEN** the user drags the edges or corners of the visualization window
- **THEN** the window resizes freely in both dimensions

#### Scenario: Visualization window minimum size
- **WHEN** the user attempts to resize the visualization window below 800x600 pixels
- **THEN** the window stops resizing and maintains at least 800x600 dimensions

#### Scenario: Visualization window closed by default
- **WHEN** the application starts
- **THEN** only the main window is visible; the visualization window is not open

#### Scenario: Visualization window can be closed independently
- **WHEN** the user closes the visualization window
- **THEN** the main window remains open and functional

#### Scenario: Visualization window has no title bar
- **WHEN** the visualization window opens
- **THEN** it displays without native title bar decorations, matching the main window style

#### Scenario: Opening already-open visualization window focuses it
- **WHEN** the user double-clicks the mini waveform while the visualization window is already open
- **THEN** the existing visualization window is focused instead of opening a new window

### Requirement: Mini Waveform Display
The main window SHALL display a miniature real-time waveform visualization in the header area next to the application logo.

#### Scenario: Mini waveform position and alignment
- **WHEN** the main window renders
- **THEN** the mini waveform appears immediately to the right of the logo, vertically centered with the logo

#### Scenario: Mini waveform size
- **WHEN** the mini waveform renders
- **THEN** its height is proportional to the logo height and its width provides adequate visualization (~120 pixels wide)

#### Scenario: Mini waveform appearance
- **WHEN** the mini waveform renders
- **THEN** it displays a gray waveform line on a transparent background with no scale, axis labels, or grid lines

#### Scenario: Mini waveform animates in real-time
- **WHEN** audio monitoring or transcription is active
- **THEN** the mini waveform displays scrolling audio amplitude in real-time, matching the time window of the full waveform (~80ms)

#### Scenario: Mini waveform idle state
- **WHEN** audio monitoring is not active
- **THEN** the mini waveform displays a flat gray line or empty state

#### Scenario: Mini waveform opens visualization window
- **WHEN** the user double-clicks the mini waveform
- **THEN** the visualization window opens (or focuses if already open)

### Requirement: Visualization Window Drag Region
The visualization window background SHALL be draggable to allow window repositioning without a native title bar.

#### Scenario: User drags visualization window via background
- **WHEN** the user clicks and drags on any non-interactive background area of the visualization window
- **THEN** the window moves with the cursor to reposition on screen

#### Scenario: Interactive elements in visualization window remain functional
- **WHEN** the user clicks on a canvas in the visualization window
- **THEN** the element receives the click event normally without initiating window drag
