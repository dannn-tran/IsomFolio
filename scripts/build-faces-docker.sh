#!/bin/bash
set -euo pipefail

# Build the Faces inference engine Docker image (linux/amd64).
#
# Needed on Intel macOS, where ONNX Runtime 1.26.0 has no native osx-x64
# library. The IsomFolio host runs this image on that platform; on all others
# it runs the native binary directly.
#
# Usage: ./scripts/build-faces-docker.sh

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="isomfolio-faces-inference:latest"

cd "$REPO_ROOT/extensions-cs"
echo ">>> Building $IMAGE (linux/amd64)"
docker build --platform linux/amd64 -f Faces/Dockerfile -t "$IMAGE" .

echo
echo "Built $IMAGE"
echo "The IsomFolio host runs it automatically on Intel macOS when you Find People."
