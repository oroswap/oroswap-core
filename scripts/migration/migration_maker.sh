#!/usr/bin/env bash
set -euo pipefail

# ─── Chain configuration ─────────────────────────────────────────────────────
export BINARY="zigchaind"
export RPC_URL="https://devnet-rpc.zigchain.com"
export CHAIN_ID="zig-devnet-1"
export KEY_NAME="devnet-key"
export KEYRING_BACKEND="test"

# ─── Transaction settings ────────────────────────────────────────────────────
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="20000uzig"
export SLEEP_TIME=5

# ─── Migration parameters ────────────────────────────────────────────────────
# Current maker contract address to migrate
export MAKER_CONTRACT="zig1fplyf3xrtfvcqjew4hnln3a3y4syzltrkjuggscq60v3hudyzhdqtrhu8d"
# New code ID to migrate the maker contract to
export NEW_MAKER_CODE_ID="151"
# Migration message (customize if your contract requires init params)
MIGRATE_MSG='{}'

# ─── Execute migration ───────────────────────────────────────────────────────
echo "Migrating maker contract $MAKER_CONTRACT to code ID $NEW_MAKER_CODE_ID..."

# Broadcast migration tx asynchronously and capture tx hash
broadcast_resp=$($BINARY tx wasm migrate "$MAKER_CONTRACT" "$NEW_MAKER_CODE_ID" "$MIGRATE_MSG" \
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

if [[ -z "$txhash" || "$txhash" == "null" ]]; then
  echo "Error: migration broadcast failed. Response: $broadcast_resp" >&2
  exit 1
fi

echo "Migration broadcasted with tx hash: $txhash"
echo "Waiting for transaction to be included..."
sleep "$SLEEP_TIME"

echo -e "\nMaker contract migration complete." 