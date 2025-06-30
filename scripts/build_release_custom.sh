#!/usr/bin/env bash

set -e
set -o pipefail

projectPath=$(cd "$(dirname "${0}")" && cd ../ && pwd)

CONTAINER_COMMAND=$(cat <<'EOF_INNER'
  # Explicitly create the /opt-artifacts directory at the root level inside the container.
  mkdir -p /opt-artifacts;

  # Set RUSTFLAGS to explicitly enable required WASM features for the compiler.
  #export RUSTFLAGS='-C target-feature=+bulk-memory,+sign-ext'; 
  export RUSTFLAGS='-C link-arg=-s'

  
  # Change to the contract's source directory and run 'bob' for building.
  echo 'Running /usr/local/bin/bob for building...'; 
  cd /code/contracts/tokenomics/incentives && /usr/local/bin/bob; 
  
  # Locate the unoptimized WASM file from the global /target directory.
  UNOPTIMIZED_WASM_PATH=$(find /target/wasm32-unknown-unknown/release -maxdepth 1 -name '*.wasm' -print -quit); 
  
  if [ -z "$UNOPTIMIZED_WASM_PATH" ]; then 
    echo 'Error: No unoptimized WASM file found in /target/wasm32-unknown-unknown/release/. Check your build configuration or bob output.'; 
    exit 1; 
  fi; 
  
  # Extract the filename.
  CONTRACT_WASM_FILENAME=$(basename "$UNOPTIMIZED_WASM_PATH"); 
  
  # Define the path for the optimized WASM file in /opt-artifacts.
  OPTIMIZED_WASM_PATH="/opt-artifacts/$CONTRACT_WASM_FILENAME"; 
  
  # --- TOOL PATHS ---
  WASM_OPT_BIN="/usr/local/bin/wasm-opt";
  COSMWASM_CHECK_BIN="/usr/local/cargo/bin/cosmwasm-check";


  # Check if wasm-opt exists.
  if [ ! -f "$WASM_OPT_BIN" ]; then
    echo "Error: wasm-opt not found at '$WASM_OPT_BIN'. Ensure optimizer image includes it.";
    exit 1;
  fi;

  echo 'Optimizing ' "$CONTRACT_WASM_FILENAME" '...'; 
  # Removed --signext-lowering as it's deprecated/unnecessary with newer Rust/wasm-opt versions (like Rust 1.86.0).
  "$WASM_OPT_BIN" \
    -Os \
    --signext-lowering \
    "$UNOPTIMIZED_WASM_PATH" \
    -o "$OPTIMIZED_WASM_PATH"; 
  
  # Check if cosmwasm-check exists.
  if [ ! -f "$COSMWASM_CHECK_BIN" ]; then
    echo "Error: cosmwasm-check not found at '$COSMWASM_CHECK_BIN'. Ensure optimizer image includes it.";
    exit 1;
  fi;

  echo 'Validation with cosmwasm-check...'; 
  "$COSMWASM_CHECK_BIN" "$OPTIMIZED_WASM_PATH"; 
  
  # Check if the optimized file was actually created.
  if [ ! -f "$OPTIMIZED_WASM_PATH" ]; then
    echo "Error: Optimized WASM file was not created at '$OPTIMIZED_WASM_PATH'.";
    exit 1;
  fi;

  echo 'Optimization and validation complete. Final Wasm at '$OPTIMIZED_WASM_PATH; 
EOF_INNER
)

# IMPORTANT: The following docker run command MUST be on a single logical line in your script file.
docker run --rm --entrypoint /bin/sh -v "$projectPath":/code --mount type=volume,source="$(basename "$projectPath")_cache",target=/target --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry -v "$projectPath/opt-artifacts":/opt-artifacts nightly-optimizer:latest -c "$CONTAINER_COMMAND"

echo "Build script finished."