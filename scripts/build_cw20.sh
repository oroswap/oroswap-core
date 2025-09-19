#!/usr/bin/env bash
set -euo pipefail

# ── 0) Locate script & project root ───────────────────────────────────────
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
OUTPUT_DIR="$project_root/artifacts"
mkdir -p "$OUTPUT_DIR"

echo "🚀 Building CW20 Base Contract..."
echo "   Output dir: $OUTPUT_DIR"

# ── 1) Clone and build CW20 ───────────────────────────────────────────────
CW20_DIR="/tmp/cw20-build"
if [[ ! -d "$CW20_DIR" ]]; then
  echo "📦 Cloning CW20 repository..."
  git clone https://github.com/CosmWasm/cw-plus.git "$CW20_DIR"
fi

cd "$CW20_DIR"
echo "🔨 Building CW20 with Docker optimizer..."

# Use the same Docker optimizer for consistency (workspace-optimizer)
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.17.0

# Copy CW20 WASM to output directory
cp artifacts/cw20_base.wasm "$OUTPUT_DIR/"
echo "✅ CW20 Base contract built and copied to $OUTPUT_DIR"

# Generate checksum for CW20
echo "🔐 Generating SHA-256 hash for CW20:"
hash_output=$(shasum -a 256 "$OUTPUT_DIR/cw20_base.wasm")
echo "$hash_output"
hash_only=$(echo "$hash_output" | cut -d' ' -f1)
echo "$hash_only  cw20_base.wasm" >> "$OUTPUT_DIR/checksums.txt"

cd "$project_root"
echo "🎉 CW20 build completed!"
echo "📋 Checksum added to: $OUTPUT_DIR/checksums.txt"
