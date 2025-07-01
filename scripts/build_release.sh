#!/usr/bin/env bash
set -euo pipefail

# ── 0) Locate script & project root ───────────────────────────────────────
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
OUTPUT_DIR="$project_root/opt-artifacts"
mkdir -p "$OUTPUT_DIR"

# ── 1) Check wasm-opt version for consistency ─────────────────────────────
if ! command -v wasm-opt &>/dev/null; then
  echo "❌ wasm-opt not found; install Binaryen tools" >&2
  exit 1
fi

WASM_OPT_VERSION=$(wasm-opt --version | cut -d' ' -f3)
EXPECTED_WASM_OPT_VERSION="123"

if [[ "$WASM_OPT_VERSION" != "$EXPECTED_WASM_OPT_VERSION" ]]; then
  echo "❌ Error: wasm-opt version mismatch!" >&2
  echo "   Expected: $EXPECTED_WASM_OPT_VERSION" >&2
  echo "   Found: $WASM_OPT_VERSION" >&2
  echo "   This will cause different SHA hashes across environments." >&2
  echo "   Please install wasm-opt version $EXPECTED_WASM_OPT_VERSION for reproducible builds." >&2
  exit 1
fi

# Display toolchain versions for reproducibility
echo "📦 Toolchain Versions:"
echo "  rustc        : $(rustc --version)"
echo "  cargo        : $(cargo --version)"
echo "  wasm-opt     : $(wasm-opt --version)"
echo "  cosmwasm-check: $(cosmwasm-check --version || echo 'not installed')"
echo ""

# ── 2) Ensure nightly + wasm32 target ─────────────────────────────────────
if ! command -v rustup &>/dev/null; then
  echo "❌ rustup not found; install from https://rustup.rs/" >&2
  exit 1
fi
rustup install nightly
rustup default nightly
rustup target add wasm32-unknown-unknown

# ── 3) Set deterministic build environment ────────────────────────────────
export RUSTFLAGS="-C target-cpu=generic -C codegen-units=1 -C target-feature=+crt-static"
export CARGO_PROFILE_RELEASE_OPT_LEVEL=3
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_PANIC="abort"
export CARGO_PROFILE_RELEASE_STRIP=true
export CARGO_PROFILE_RELEASE_DEBUG=false
export CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=false

# Remove any existing build artifacts to ensure clean build
rm -rf "$project_root/target"

# ── 4) Find all contract Cargo.toml files ─────────────────────────────────
contract_manifests=($(find "$project_root/contracts" -name "Cargo.toml"))

# ── 5) Build each contract ────────────────────────────────────────────────
for manifest_path in "${contract_manifests[@]}"; do
  # Get package name from manifest
  pkg_name=$(awk -F '"' '/^\[package\]/{p=1} p && /^name =/ {print $2; exit}' "$manifest_path")
  echo "🚀 Building contract '$pkg_name' from manifest $manifest_path"

  # Check if contract has zigchain feature
  if grep -q "zigchain" "$manifest_path"; then
    echo "📦 Compiling $pkg_name with zigchain feature..."
    cargo build --release --target wasm32-unknown-unknown --manifest-path "$manifest_path" --features zigchain --no-default-features
  else
    echo "📦 Compiling $pkg_name without zigchain feature..."
    cargo build --release --target wasm32-unknown-unknown --manifest-path "$manifest_path"
  fi

  # Locate raw Wasm
  raw_wasm="$project_root/target/wasm32-unknown-unknown/release/${pkg_name//-/_}.wasm"
  if [[ ! -f "$raw_wasm" ]]; then
    echo "❌ Raw Wasm not found: $raw_wasm" >&2
    continue
  fi

  echo "🔍 Found raw Wasm: $raw_wasm"

  # Optimize & validate with deterministic settings
  final_wasm="$OUTPUT_DIR/${pkg_name//-/_}.wasm"
  echo "🔎 Optimizing & lowering bulk-memory → $final_wasm"
  wasm-opt -Os --enable-bulk-memory --llvm-memory-copy-fill-lowering --signext-lowering \
    --strip-debug --strip-producers --strip-target-features \
    "$raw_wasm" -o "$final_wasm"

  echo "✅ Validating with cosmwasm-check"
  cosmwasm-check "$final_wasm"

  # Generate and display hash for verification
  echo "🔐 Generating SHA-256 hash for verification:"
  shasum -a 256 "$final_wasm"

  echo "🎉 Build complete for $pkg_name — optimized Wasm at $final_wasm"
done

echo "✨ All contracts built successfully!"
