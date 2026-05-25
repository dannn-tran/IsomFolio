#!/bin/bash
set -euo pipefail

# Build and package an addon into a .isfx distribution file.
# Usage: ./scripts/package-addon.sh <addon-dir> [--release]
#
# Examples:
#   ./scripts/package-addon.sh addons/autotag-clip
#   ./scripts/package-addon.sh addons/faces --release

ADDON_DIR="${1:?Usage: $0 <addon-dir> [--release]}"
RELEASE_FLAG="${2:-}"

if [ ! -f "$ADDON_DIR/manifest.json" ]; then
    echo "Error: $ADDON_DIR/manifest.json not found"
    exit 1
fi

ADDON_NAME=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['name'])" "$ADDON_DIR/manifest.json")
BIN_NAME=$(grep -A1 '^\[\[bin\]\]' "$ADDON_DIR/Cargo.toml" | grep 'name' | sed 's/.*"\(.*\)".*/\1/')

if [ -z "$BIN_NAME" ]; then
    BIN_NAME="$ADDON_NAME"
fi

echo "Building $ADDON_NAME (binary: $BIN_NAME)..."

BUILD_ARGS="-p $(grep '^name' "$ADDON_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
if [ "$RELEASE_FLAG" = "--release" ]; then
    BUILD_ARGS="$BUILD_ARGS --release"
    PROFILE="release"
else
    PROFILE="debug"
fi

cargo build $BUILD_ARGS

BINARY="target/$PROFILE/$BIN_NAME"
if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

DIST_DIR="addons/dist"
mkdir -p "$DIST_DIR"
OUTPUT="$DIST_DIR/$ADDON_NAME.isfx"

zip -j "$OUTPUT" "$ADDON_DIR/manifest.json" "$BINARY"

echo "Packaged: $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
