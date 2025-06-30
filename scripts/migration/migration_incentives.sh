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
# Current incentives contract address to migrate
export INCENTIVES_CONTRACT="zig1f5u0qsu76gjnwrm6pmvztwjt8n98jq7qpfgugjq3fxrvccczxudqax3ld6"
# New code ID to migrate the incentives contract to
export NEW_INCENTIVES_CODE_ID="39"

# **Required** new init params:
# - oro_token: the native denom or CW20 you now use instead of astro_token
# - vesting_contract: your existing vesting contract address
# (remove or rename these if you’re using a CW20 oro_token)
export ORO_DENOM="uzig"
export VESTING_CONTRACT="zig1…your-vesting-address…"

# Build the migrate message with only the non‐optional fields
MIGRATE_MSG=$(
  jq -nc \
    --arg denom   "$ORO_DENOM" \
    --arg vesting "$VESTING_CONTRACT" \
    '{
      oro_token: { native_token: { denom: $denom } },
      vesting_contract: $vesting
    }'
)

# ─── Execute migration ───────────────────────────────────────────────────────
echo "Migrating $INCENTIVES_CONTRACT → code ID $NEW_INCENTIVES_CODE_ID"
echo "MIGRATE_MSG: $MIGRATE_MSG"

broadcast_resp=$(
  $BINARY tx wasm migrate "$INCENTIVES_CONTRACT" "$NEW_INCENTIVES_CODE_ID" "$MIGRATE_MSG" \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    --broadcast-mode async \
    -y -o json
)

txhash=$(echo "$broadcast_resp" | jq -r .txhash)
if [[ -z "$txhash" || "$txhash" == "null" ]]; then
  echo "❌ Migration broadcast failed. Response:" >&2
  echo "$broadcast_resp" >&2
  exit 1
fi

echo "✅ Migration tx submitted: $txhash"
echo "⏳ Waiting $SLEEP_TIME s for inclusion…"
sleep "$SLEEP_TIME"
echo "🎉 Incentives contract migrated to code ID $NEW_INCENTIVES_CODE_ID"
