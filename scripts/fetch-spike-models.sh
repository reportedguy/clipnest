#!/usr/bin/env bash
# Downloads llama.cpp (CPU) and the candidate models for spikes 4-5, resumably, and
# verifies each file's SHA256 against Hugging Face's LFS metadata.
#   scripts/fetch-spike-models.sh <dest-dir>
set -euo pipefail
DEST="${1:?usage: fetch-spike-models.sh <dest-dir>}"
LLAMA_TAG="b11669"
mkdir -p "$DEST"

fetch_hf() { # repo file
  local repo="$1" file="$2" out="$DEST/$2"
  local sha
  sha=$(curl -sf "https://huggingface.co/api/models/$repo/tree/main" |
    python -c "import sys,json; print(next(f['lfs']['oid'] for f in json.load(sys.stdin) if f['path']==sys.argv[1]))" "$file")
  if [ -f "$out" ] && [ "$(sha256sum "$out" | cut -d' ' -f1)" = "$sha" ]; then
    echo "ok (cached) $file"; return
  fi
  curl -fL --retry 5 -C - -o "$out" "https://huggingface.co/$repo/resolve/main/$file"
  [ "$(sha256sum "$out" | cut -d' ' -f1)" = "$sha" ] && echo "ok $file $sha" || { echo "CHECKSUM MISMATCH $file"; exit 1; }
}

zip="llama-$LLAMA_TAG-bin-win-cpu-x64.zip"
if [ ! -f "$DEST/llama/llama-server.exe" ]; then
  curl -fL --retry 5 -o "$DEST/$zip" "https://github.com/ggml-org/llama.cpp/releases/download/$LLAMA_TAG/$zip"
  mkdir -p "$DEST/llama" && unzip -oq "$DEST/$zip" -d "$DEST/llama"
fi
echo "llama.cpp $LLAMA_TAG ready"

fetch_hf Qwen/Qwen2.5-3B-Instruct-GGUF        qwen2.5-3b-instruct-q4_k_m.gguf
fetch_hf bartowski/Llama-3.2-3B-Instruct-GGUF Llama-3.2-3B-Instruct-Q4_K_M.gguf
fetch_hf unsloth/Qwen3.5-4B-GGUF              Qwen3.5-4B-Q4_K_M.gguf
fetch_hf unsloth/Qwen3.5-4B-GGUF              mmproj-F16.gguf
fetch_hf ggml-org/gemma-3-4b-it-qat-GGUF      gemma-3-4b-it-qat-Q4_0.gguf
fetch_hf ggml-org/gemma-3-4b-it-qat-GGUF      mmproj-model-f16-4B.gguf
echo "ALL DONE"
