#!/usr/bin/env bash
set -euo pipefail

# ── 0) Locate script & project root ───────────────────────────────────────
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
OUTPUT_DIR="$project_root/opt-artifacts"
mkdir -p "$OUTPUT_DIR"

# ── 1) Ensure nightly + wasm32 target ─────────────────────────────────────
if ! command -v rustup &>/dev/null; then
  echo "❌ rustup not found; install from https://rustup.rs/" >&2
  exit 1
fi
rustup install nightly
rustup default nightly
rustup target add wasm32-unknown-unknown

# ── 2) Set the contract name here ───────────────────────────────────────
#contract_name="periphery/pool_initializer"  # Updated to build the pool initializer contract
#contract_name="tokenomics/incentives" 
contract_name="tokenomics/maker" 
#contract_name="periphery/native_coin_registry" 
#contract_name="router" 

# ── 3) Find the contract Cargo.toml file ─────────────────────────────────
contract_manifest="$project_root/contracts/$contract_name/Cargo.toml"
if [[ ! -f "$contract_manifest" ]]; then
  echo "❌ Contract manifest not found: $contract_manifest" >&2
  exit 1
fi

# ── 4) Build the contract ────────────────────────────────────────────────
echo "🚀 Building contract '$contract_name' from manifest $contract_manifest"

# Check if contract has zigchain feature
if grep -q "zigchain" "$contract_manifest"; then
  echo "📦 Compiling $contract_name with zigchain feature..."
  cargo build --release --target wasm32-unknown-unknown --manifest-path "$contract_manifest" --features zigchain --no-default-features
else
  echo "📦 Compiling $contract_name without zigchain feature..."
  cargo build --release --target wasm32-unknown-unknown --manifest-path "$contract_manifest"
fi

# ── 5) Find the generated Wasm file ─────────────────────────────────────
# Look for the Wasm file in the target directory, excluding the deps directory
# For tokenomics/maker, the wasm file is named oroswap_maker.wasm
if [[ "$contract_name" == "tokenomics/maker" ]]; then
  raw_wasm=$(find "$project_root/target/wasm32-unknown-unknown/release" -maxdepth 1 -name "oroswap_maker.wasm" -type f)
elif [[ "$contract_name" == "tokenomics/incentives" ]]; then
  raw_wasm=$(find "$project_root/target/wasm32-unknown-unknown/release" -maxdepth 1 -name "oroswap_incentives.wasm" -type f)
elif [[ "$contract_name" == "periphery/pool_initializer" ]]; then
  raw_wasm=$(find "$project_root/target/wasm32-unknown-unknown/release" -maxdepth 1 -name "oroswap_pool_initializer.wasm" -type f)
elif [[ "$contract_name" == "periphery/native_coin_registry" ]]; then
  raw_wasm=$(find "$project_root/target/wasm32-unknown-unknown/release" -maxdepth 1 -name "oroswap_native_coin_registry.wasm" -type f)
else
  raw_wasm=$(find "$project_root/target/wasm32-unknown-unknown/release" -maxdepth 1 -name "*${contract_name//\//_}.wasm" -type f)
fi

if [[ -z "$raw_wasm" ]]; then
  echo "❌ Raw Wasm not found for contract: $contract_name" >&2
  exit 1
fi

echo "🔍 Found raw Wasm: $raw_wasm"

# ── 6) Optimize & validate ─────────────────────────────────────────────
# Use the same filename for the optimized version
final_wasm="$OUTPUT_DIR/$(basename "$raw_wasm")"
echo "🔎 Optimizing & lowering bulk-memory → $final_wasm"
wasm-opt -Os --enable-bulk-memory --llvm-memory-copy-fill-lowering --signext-lowering \
  "$raw_wasm" -o "$final_wasm"

echo "✅ Validating with cosmwasm-check"
cosmwasm-check "$final_wasm"

echo "🎉 Build complete for $contract_name — optimized Wasm at $final_wasm" 