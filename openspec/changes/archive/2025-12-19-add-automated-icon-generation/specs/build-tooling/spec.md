## ADDED Requirements

### Requirement: Icon Generation Script
The build tooling SHALL provide a script to automatically generate all required application icons from a single source SVG file.

#### Scenario: Generate icons from source SVG
- **WHEN** the developer runs `pnpm icons:generate`
- **THEN** the script converts `images/flowstt-icon.svg` to a temporary 1024x1024 PNG
- **AND** invokes `tauri icon` to generate all platform-specific icons in `src-tauri/icons/`
- **AND** cleans up the temporary PNG file

#### Scenario: Missing ImageMagick dependency
- **WHEN** ImageMagick is not installed on the system
- **THEN** the script exits with an error message directing the user to install ImageMagick

#### Scenario: Missing source SVG
- **WHEN** the source SVG file does not exist at `images/flowstt-icon.svg`
- **THEN** the script exits with an error message indicating the missing file

### Requirement: NPM Script Integration
The build tooling SHALL expose icon generation through the npm/pnpm scripts interface.

#### Scenario: Icons generate script available
- **WHEN** the developer runs `pnpm icons:generate`
- **THEN** the `scripts/generate-icons.sh` script is executed
