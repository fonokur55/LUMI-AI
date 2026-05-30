#!/usr/bin/env bash
# =============================================================================
#  v0.2.0 bundled Szöveg expert letöltő (Gemma 2 2B-it Q4_K_M) — macOS/Linux
# =============================================================================
#  RELEASE BUILD ELŐTT FUTTATNI KELL — különben a telepítő nem tartalmazza
#  a Gemma modellt, és az első indításnál csak letöltési-flow lesz.
#
#  HASZNÁLAT:
#    chmod +x scripts/fetch-bundled-szoveg.sh
#    ./scripts/fetch-bundled-szoveg.sh
#
#  EREDMÉNY:
#    atman/src-tauri/resources/szoveg.gguf (~1.6 GB)
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESOURCES="$REPO_ROOT/atman/src-tauri/resources"
TARGET="$RESOURCES/szoveg.gguf"

URL='https://huggingface.co/bartowski/gemma-2-2b-it-GGUF/resolve/main/gemma-2-2b-it-Q4_K_M.gguf'

if [ -f "$TARGET" ]; then
  size_bytes=$(stat -f%z "$TARGET" 2>/dev/null || stat -c%s "$TARGET" 2>/dev/null || echo 0)
  size_mb=$((size_bytes / 1024 / 1024))
  if [ "$size_mb" -gt 1000 ]; then
    echo "OK: szoveg.gguf már létezik (${size_mb} MB), kihagyom a letöltést."
    echo "    Töröld le ($TARGET), ha újra le akarod tölteni."
    exit 0
  else
    echo "FIGYELEM: szoveg.gguf túl kicsi (${size_mb} MB), valószínűleg sérült. Újraletöltés..."
    rm -f "$TARGET"
  fi
fi

mkdir -p "$RESOURCES"

echo "Letöltés: $URL"
echo "Cél:      $TARGET"
echo "Méret:    ~1.6 GB (Gemma 2 2B-it Q4_K_M)"
echo ""

tmp="$TARGET.part"
if command -v curl >/dev/null 2>&1; then
  curl -L -o "$tmp" "$URL" --progress-bar
elif command -v wget >/dev/null 2>&1; then
  wget -O "$tmp" "$URL"
else
  echo "HIBA: nem találtam sem curl-t sem wget-et." >&2
  exit 1
fi

size_bytes=$(stat -f%z "$tmp" 2>/dev/null || stat -c%s "$tmp")
size_mb=$((size_bytes / 1024 / 1024))
if [ "$size_mb" -lt 1000 ]; then
  rm -f "$tmp"
  echo "HIBA: letöltött fájl túl kicsi (${size_mb} MB) — valószínűleg sérült." >&2
  exit 1
fi

mv "$tmp" "$TARGET"
echo ""
echo "OK: szoveg.gguf letöltve (${size_mb} MB)."
echo "    Most már futtathatod: cd atman && npm run tauri build"
