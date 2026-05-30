#!/bin/bash
set -euo pipefail

# Copy `.isfx` artifacts from extension dist/ directories into the integration
# test fixtures dir so `cargo test -p isomfolio-extension-host` can pick them up.
#
# Usage:
#   ./scripts/sync-test-fixtures.sh

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$REPO_ROOT/isomfolio-extension-host/tests/fixtures"
mkdir -p "$FIXTURES"

# Source dist directories. Add more here as other extension flavours appear.
SOURCES=(
    "$REPO_ROOT/extensions-cs/dist"
    "$REPO_ROOT/extensions/dist"
)

copied=0
for src in "${SOURCES[@]}"; do
    [ -d "$src" ] || continue
    while IFS= read -r -d '' isfx; do
        # Prefer the generic <name>.isfx over arch-specific variants.
        case "$(basename "$isfx")" in
            *-osx-x64.isfx|*-osx-arm64.isfx|*-linux-x64.isfx|*-linux-arm64.isfx|*-win-x64.isfx)
                continue
                ;;
        esac
        cp "$isfx" "$FIXTURES/"
        echo "  copied $(basename "$isfx") ← $src"
        copied=$((copied + 1))
    done < <(find "$src" -maxdepth 1 -name "*.isfx" -print0)
done

if [ "$copied" -eq 0 ]; then
    echo "No .isfx found in any dist/ directory."
    echo "Run a build script first, e.g. ./scripts/build-faces.sh"
    exit 1
fi

echo
echo "Synced $copied package(s) into $FIXTURES"
echo "Run: cargo test -p isomfolio-extension-host"
