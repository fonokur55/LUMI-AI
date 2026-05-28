#!/usr/bin/env bash
# Downloads AKASHA Phase 2 arsenal (eco / brain / creative) to models/akasha/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIR="$ROOT/models/akasha"
mkdir -p "$DIR"

ECO_ONLY=0
SKIP_ECO=0
BRAIN_REPO="${BRAIN_REPO:-Qwen/Qwen2.5-Coder-7B-Instruct-GGUF}"
BRAIN_FILE="${BRAIN_FILE:-qwen2.5-coder-7b-instruct-q4_k_m.gguf}"
CREATIVE_REPO="${CREATIVE_REPO:-dphn/dolphin-2.9.4-llama3.1-8b-gguf}"
CREATIVE_FILE="${CREATIVE_FILE:-dolphin-2.9.4-llama3.1-8b-Q4_K_M.gguf}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --eco-only) ECO_ONLY=1; shift ;;
    --skip-eco) SKIP_ECO=1; shift ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

hf_url() {
  echo "https://huggingface.co/$1/resolve/main/$2"
}

download_file() {
  local url="$1"
  local out="$2"
  local min_bytes=52428800
  local size=0
  if [[ -f "$out" ]]; then
    size=$(stat -f%z "$out" 2>/dev/null || stat -c%s "$out")
    if [[ "$size" -gt "$min_bytes" ]]; then
      echo "Already exists, skip: $out"
      return
    fi
    echo "Corrupt/small file, re-downloading: $out"
    rm -f "$out"
  fi
  echo "Downloading: $url"
  echo "Target: $out"
  curl -L --retry 5 --retry-delay 5 -C - -o "${out}.part" "$url"
  size=$(stat -f%z "${out}.part" 2>/dev/null || stat -c%s "${out}.part")
  if [[ "$size" -lt "$min_bytes" ]]; then
    rm -f "${out}.part"
    echo "Download too small ($size bytes) - bad URL or HF error" >&2
    exit 1
  fi
  mv -f "${out}.part" "$out"
  echo "Done: $out"
}

ECO_OUT="$DIR/eco.Q4_K_M.gguf"
LEGACY="$DIR/akasha-moe.Q4_K_M.gguf"

if [[ "$SKIP_ECO" -eq 0 ]]; then
  if [[ -f "$ECO_OUT" ]]; then
    echo "Eco model exists: $ECO_OUT"
  elif [[ -f "$LEGACY" ]]; then
    echo "Eco: copy from legacy..."
    cp -f "$LEGACY" "$ECO_OUT"
  else
    # NOTE: the previous DeepSeek-distill-based MoE was a reasoning model;
    # at this small size (~4B total params) it fell into infinite thinking
    # loops on casual chat ("DeepSeek10", "fitted with whom?"). Replaced
    # with a clean instruction-following chat model — same speed class,
    # multilingual, no reasoning loops.
    ECO_REPO="Qwen/Qwen2.5-3B-Instruct-GGUF"
    ECO_FILE="qwen2.5-3b-instruct-q4_k_m.gguf"
    download_file "$(hf_url "$ECO_REPO" "$ECO_FILE")" "$ECO_OUT"
  fi
fi

[[ "$ECO_ONLY" -eq 1 ]] && exit 0

download_file "$(hf_url "$BRAIN_REPO" "$BRAIN_FILE")" "$DIR/brain.Q4_K_M.gguf"
download_file "$(hf_url "$CREATIVE_REPO" "$CREATIVE_FILE")" "$DIR/creative.Q4_K_M.gguf"

echo ""
echo "Arsenal ready:"
ls -lh "$DIR"/*.gguf 2>/dev/null || true
