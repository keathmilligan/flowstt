#!/usr/bin/env bash
#
# Generate all application icons from the source images.
# Requires: ImageMagick (magick command)
#
# App bundle icons (taskbar, window, installer, etc.) are generated from
# images/app-icon.png (or app-icon.svg as fallback).
#
# The system tray icon is generated separately from images/flowstt-icon.svg
# and placed at src-tauri/icons/tray/icon.png.
#
# Usage: ./scripts/generate-icons.sh
#        pnpm icons:generate

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

APP_ICON_PNG="$PROJECT_ROOT/images/app-icon.png"
APP_ICON_SVG="$PROJECT_ROOT/images/app-icon.svg"
TRAY_ICON_SVG="$PROJECT_ROOT/images/flowstt-icon.svg"
TEMP_PNG="$PROJECT_ROOT/images/app-icon-temp.png"
TRAY_TEMP_PNG="$PROJECT_ROOT/images/tray-icon-temp.png"
ICON_OUTPUT_DIR="$PROJECT_ROOT/src-tauri/icons"

# Check for ImageMagick
if ! command -v magick &> /dev/null; then
    echo "Error: ImageMagick is not installed or 'magick' command not found."
    echo "Please install ImageMagick: https://imagemagick.org/script/download.php"
    exit 1
fi

# Resolve app icon source: prefer PNG, fall back to SVG
if [[ -f "$APP_ICON_PNG" ]]; then
    echo "Using app icon source: $APP_ICON_PNG"
    APP_ICON_SOURCE="$APP_ICON_PNG"
elif [[ -f "$APP_ICON_SVG" ]]; then
    echo "Using app icon source: $APP_ICON_SVG (SVG)"
    APP_ICON_SOURCE="$APP_ICON_SVG"
else
    echo "Error: No app icon source found. Expected one of:"
    echo "  $APP_ICON_PNG"
    echo "  $APP_ICON_SVG"
    exit 1
fi

# Check tray icon source exists
if [[ ! -f "$TRAY_ICON_SVG" ]]; then
    echo "Error: Tray icon source SVG not found at $TRAY_ICON_SVG"
    exit 1
fi

# --- App bundle icons ---
echo "Converting app icon source to 1024x1024 PNG..."
magick -background none "$APP_ICON_SOURCE" -resize 1024x1024 "$TEMP_PNG"

echo "Generating Tauri bundle icons..."
cd "$PROJECT_ROOT"
pnpm tauri icon "$TEMP_PNG" --output "$ICON_OUTPUT_DIR"

echo "Cleaning up temporary file..."
rm -f "$TEMP_PNG"

# --- Tray icon (overwrite what Tauri generated) ---
echo "Generating tray icon from $TRAY_ICON_SVG..."
magick -background none "$TRAY_ICON_SVG" -resize 32x32 "$TRAY_TEMP_PNG"
mkdir -p "$ICON_OUTPUT_DIR/tray"
cp "$TRAY_TEMP_PNG" "$ICON_OUTPUT_DIR/tray/icon.png"
rm -f "$TRAY_TEMP_PNG"

echo "Done! Icons generated in $ICON_OUTPUT_DIR"
echo "  Bundle icons: from $(basename "$APP_ICON_SOURCE")"
echo "  Tray icon:    from $(basename "$TRAY_ICON_SVG") -> icons/tray/icon.png"
