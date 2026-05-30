#!/bin/bash
set -euo pipefail

# Build and package a Rust extension into a .isfx distribution file.
# Usage: ./scripts/package-extension.sh <extension-dir> [--release]
#
# Examples:
#   ./scripts/package-extension.sh extensions/autotag-clip
#   ./scripts/package-extension.sh extensions/autotag-openai --release
#
# For the C# Faces extension, see ./scripts/build-faces.sh instead.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXT_DIR="${1:?Usage: $0 <extension-dir> [--release]}"
RELEASE_FLAG="${2:-}"

if [ ! -f "$EXT_DIR/manifest.json" ]; then
    echo "Error: $EXT_DIR/manifest.json not found"
    exit 1
fi

PACKAGE_NAME=$(grep '^name' "$EXT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
BIN_NAME=$(grep -A1 '^\[\[bin\]\]' "$EXT_DIR/Cargo.toml" | grep 'name' | sed 's/.*"\(.*\)".*/\1/' || true)
BIN_NAME="${BIN_NAME:-$PACKAGE_NAME}"

echo "Building $PACKAGE_NAME (binary: $BIN_NAME)..."

if [ "$RELEASE_FLAG" = "--release" ]; then
    PROFILE="release"
    cargo build -p "$PACKAGE_NAME" --release
else
    PROFILE="debug"
    cargo build -p "$PACKAGE_NAME"
fi

BINARY="$REPO_ROOT/target/$PROFILE/$BIN_NAME"
if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

DIST_DIR="$REPO_ROOT/extensions/dist"
mkdir -p "$DIST_DIR"
OUTPUT="$DIST_DIR/$BIN_NAME.isfx"

zip -j "$OUTPUT" "$EXT_DIR/manifest.json" "$BINARY"

echo "Packaged: $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
