#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/runtime/macos"
EXTRACT="$TMPDIR/llama-cpp-extract-mac"
mkdir -p "$OUT"
rm -rf "$EXTRACT"
mkdir -p "$EXTRACT"

echo "llama.cpp legutobbi release lekerese..."
JSON=$(curl -sL "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest")

pick_url() {
  echo "$JSON" | python3 -c "
import json, re, sys
data = json.load(sys.stdin)
patterns = [
    r'llama-.*-bin-macos-arm64\.zip\$',
    r'llama-.*-bin-macos-x64\.zip\$',
    r'llama-.*-bin-macos.*\.zip\$',
]
for pat in patterns:
    for a in data.get('assets', []):
        name = a.get('name', '')
        if re.search(pat, name) and not name.startswith('cudart-'):
            print(a['browser_download_url'])
            sys.exit(0)
sys.exit(1)
" 2>/dev/null || true
}

URL=$(pick_url)
if [ -z "$URL" ]; then
  echo "Nem talalhato macOS binaris csomag."
  echo "Manualis letoltes: https://github.com/ggml-org/llama.cpp/releases"
  exit 1
fi

echo "Letoltes..."
curl -sL "$URL" -o "$EXTRACT/archive.zip"
unzip -q "$EXTRACT/archive.zip" -d "$EXTRACT/root"

SERVER=$(find "$EXTRACT/root" -name "llama-server" -type f | head -1)
if [ -z "$SERVER" ]; then
  echo "llama-server nem talalhato a csomagban."
  exit 1
fi

BIN_ROOT="$(dirname "$SERVER")"
cp -R "$BIN_ROOT/"* "$OUT/"
chmod +x "$OUT/llama-server"
echo "Kesz: $OUT/llama-server"
