#!/usr/bin/env bash
#
# Generate all application icons from the source SVG.
# Requires: ImageMagick (magick command)
#
# Usage: ./scripts/generate-icons.sh
#        pnpm icons:generate

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE_SVG="$PROJECT_ROOT/images/flowstt-icon.svg"
TEMP_PNG="$PROJECT_ROOT/images/flowstt-icon-temp.png"
ICON_OUTPUT_DIR="$PROJECT_ROOT/src-tauri/icons"

# Check for ImageMagick
if ! command -v magick &> /dev/null; then
    echo "Error: ImageMagick is not installed or 'magick' command not found."
    echo "Please install ImageMagick: https://imagemagick.org/script/download.php"
    exit 1
fi

# Check source SVG exists
if [[ ! -f "$SOURCE_SVG" ]]; then
    echo "Error: Source SVG not found at $SOURCE_SVG"
    exit 1
fi

echo "Converting SVG to PNG..."
magick -background none "$SOURCE_SVG" -resize 1024x1024 "$TEMP_PNG"

echo "Generating Tauri icons..."
cd "$PROJECT_ROOT"
pnpm tauri icon "$TEMP_PNG" --output "$ICON_OUTPUT_DIR"

echo "Cleaning up temporary file..."
rm -f "$TEMP_PNG"

echo "Done! Icons generated in $ICON_OUTPUT_DIR"
