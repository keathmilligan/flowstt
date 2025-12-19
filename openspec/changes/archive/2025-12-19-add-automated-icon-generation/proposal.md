# Change: Add automated app icon generation

## Why
Currently, app icons must be manually created and placed in `src-tauri/icons/`. Adding an automated generation script ensures consistent icon quality across all platforms and simplifies the process of updating the app icon—developers only need to update the source SVG.

## What Changes
- Add a bash script (`scripts/generate-icons.sh`) that converts the source SVG to all required icon formats
- Add an npm script (`icons:generate`) to invoke the generation script
- Use ImageMagick and `tauri icon` CLI to generate platform-specific icons from `images/flowstt-icon.svg`

## Impact
- Affected specs: build-tooling (new capability)
- Affected code: `package.json`, new `scripts/` directory
- No breaking changes to existing functionality
