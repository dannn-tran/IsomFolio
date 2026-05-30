#!/bin/bash
set -euo pipefail

# Publish the Faces extension via `dotnet publish` and package the output as
# `extensions-cs/dist/faces.isfx`.
#
# Usage:
#   ./scripts/build-faces.sh                # detect host arch
#   ./scripts/build-faces.sh osx-arm64      # explicit RID
#   ./scripts/build-faces.sh --all          # build for both osx-x64 and osx-arm64
#
# Supported RIDs: osx-x64, osx-arm64, linux-x64, linux-arm64, win-x64.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FACES_DIR="$REPO_ROOT/extensions-cs/Faces"
DIST_DIR="$REPO_ROOT/extensions-cs/dist"

detect_rid() {
    local os arch
    case "$(uname -s)" in
        Darwin) os=osx ;;
        Linux)  os=linux ;;
        *)      echo "unsupported host OS: $(uname -s)" >&2; exit 1 ;;
    esac
    case "$(uname -m)" in
        arm64|aarch64) arch=arm64 ;;
        x86_64)        arch=x64 ;;
        *) echo "unsupported host arch: $(uname -m)" >&2; exit 1 ;;
    esac
    echo "$os-$arch"
}

package_for_rid() {
    local rid="$1"
    echo ">>> Publishing faces for $rid"

    (cd "$FACES_DIR" && dotnet publish -c Release -r "$rid" --self-contained --nologo)

    local publish_dir="$FACES_DIR/bin/Release/net10.0/$rid/publish"
    if [ ! -d "$publish_dir" ]; then
        echo "publish output not found at $publish_dir" >&2
        exit 1
    fi

    mkdir -p "$DIST_DIR"
    local output="$DIST_DIR/faces-$rid.isfx"
    rm -f "$output"
    (cd "$publish_dir" && zip -rq "$output" . -x "*.pdb" "*.dSYM/*")

    # Also drop a generic faces.isfx pointing at the host-arch build, so
    # `extensions-cs/dist/faces.isfx` is the conventional "install me" artifact.
    if [ "$rid" = "$(detect_rid)" ]; then
        cp "$output" "$DIST_DIR/faces.isfx"
        echo "    → $DIST_DIR/faces.isfx (host-arch alias)"
    fi
    echo "    → $output ($(du -h "$output" | cut -f1))"
}

case "${1:-}" in
    --all)
        package_for_rid osx-x64
        package_for_rid osx-arm64
        ;;
    "")
        package_for_rid "$(detect_rid)"
        ;;
    *)
        package_for_rid "$1"
        ;;
esac

echo
echo "Done. Install in IsomFolio: Settings → Extensions → Install Extension… → pick $DIST_DIR/faces.isfx"
echo "Run the integration test:   ./scripts/sync-test-fixtures.sh && cargo test -p isomfolio-extension-host"
