#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ATMAN="$ROOT/atman"
OUT="$ROOT/dist/portable"

cd "$ATMAN"
npm run build
npm run tauri build

APP=$(find "$ATMAN/src-tauri/target/release/bundle" -name "ATMAN.app" -type d | head -1)
if [ -z "$APP" ]; then
  echo "ATMAN.app nem található."
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT/models/akasha" "$OUT/models/embed" "$OUT/runtime/macos"
cp -R "$APP" "$OUT/"
if [ -f "$ROOT/runtime/macos/llama-server" ]; then
  cp "$ROOT/runtime/macos/llama-server" "$OUT/runtime/macos/"
  chmod +x "$OUT/runtime/macos/llama-server"
fi
cp "$ROOT/docs/PORTABLE.md" "$OUT/README-PORTABLE.txt" 2>/dev/null || true

echo "Portable csomag: $OUT"
echo "Helyezd ide az AKASHA MoE GGUF-ot: models/akasha/akasha-moe.Q4_K_M.gguf"
