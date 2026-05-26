#!/usr/bin/env bash
# Download a whisper.cpp ggml model into src-tauri/resources/models/.
# The Tauri bundler picks the file up from there and ships it with the .app.
#
# Default model: small (466 MB) — a reasonable balance between Japanese
# accuracy and disk size. Override via:
#   MODEL=medium ./scripts/fetch-model.sh   # 1.5 GB, better Japanese
#   MODEL=base   ./scripts/fetch-model.sh   # 142 MB, faster, lower accuracy

set -euo pipefail

MODEL="${MODEL:-small}"
DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/resources/models"
DEST_FILE="$DEST_DIR/ggml-${MODEL}.bin"
URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${MODEL}.bin"

mkdir -p "$DEST_DIR"

if [ -f "$DEST_FILE" ]; then
  echo "model already present: $DEST_FILE"
  exit 0
fi

echo "downloading $MODEL model to $DEST_FILE"
if command -v curl >/dev/null 2>&1; then
  curl -L --fail --progress-bar -o "$DEST_FILE" "$URL"
elif command -v wget >/dev/null 2>&1; then
  wget --show-progress -O "$DEST_FILE" "$URL"
else
  echo "error: need curl or wget installed" >&2
  exit 1
fi

echo "done."
