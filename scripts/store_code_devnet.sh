#!/usr/bin/env bash
set -euo pipefail

# ─── Chain configuration ─────────────────────────────────────────────────────
export BINARY="zigchaind"
export RPC_URL="https://devnet-rpc.zigchain.com"
export CHAIN_ID="zig-devnet-1"
export KEY_NAME="devnet-key"
export KEYRING_BACKEND="test"

# ─── Transaction settings ────────────────────────────────────────────────────
export SLEEP_TIME=5
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="20000uzig"

# ─── Directories and output file ─────────────────────────────────────────────
ARTIFACT_DIR="../opt-artifacts"
ENV_FILE="devnet_tetsting_code_ids.env"

# Reset env file
echo "# Devnet testing code IDs" > "$ENV_FILE"

echo "Storing WASM files from $ARTIFACT_DIR to chain $CHAIN_ID..."
for wasm in "$ARTIFACT_DIR"/*.wasm; do
  filename=$(basename "$wasm" .wasm)
  echo -e "\n---> Storing $filename.wasm"

  # Broadcast asynchronously and capture tx hash
  broadcast_resp=$($BINARY tx wasm store "$wasm" \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    --broadcast-mode async \
    -y --output json)
  txhash=$(echo "$broadcast_resp" | jq -r '.txhash')

  echo "Broadcasted txhash: $txhash"
  echo "Waiting $SLEEP_TIME seconds for inclusion..."
  sleep "$SLEEP_TIME"

  # Query transaction to get full result
  tx_json=$($BINARY query tx "$txhash" \
    --node "$RPC_URL" \
    --chain-id "$CHAIN_ID" \
    --output json)

  # Extract code_id from events
  code_id=$(echo "$tx_json" | jq -r '.events[] |
    select(.type=="store_code") |
    .attributes[] |
    select(.key=="code_id") |
    .value')

  # Validate parsing
  if [[ -z "$code_id" || "$code_id" == "null" ]]; then
    echo "Error: Unable to parse code_id for $filename" >&2
    echo "Full tx JSON:" >&2
    echo "$tx_json" >&2
    exit 1
  fi

  # Format environment variable name
  var_name="$(echo "$filename" | tr '[:lower:]-' '[:upper:]_')_CODE_ID"

  # Append to env file and print
  echo "export $var_name=$code_id"
  echo "export $var_name=$code_id" >> "$ENV_FILE"

done

# Completion message
echo -e "\nAll code IDs have been written to $ENV_FILE."
