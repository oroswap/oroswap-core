#!/usr/bin/env bash
set -euo pipefail

# ─── Source devnet environment ───────────────────────────────────────────────
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/../devnet.env"

# ─── Migration parameters ────────────────────────────────────────────────────
# Current incentives contract address to migrate
export INCENTIVES_CONTRACT="$INC_CONTRACT"
# New code ID to migrate the incentives contract to
export NEW_INCENTIVES_CODE_ID="$OROSWAP_INCENTIVES_CODE_ID"

# **Required** new init params:
# - oro_token: the native denom or CW20 you now use instead of astro_token
# - vesting_contract: your existing vesting contract address
# - factory: the correct factory contract address
export ORO_DENOM="$ZIG_ADDRESS"
export VESTING_CONTRACT="zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq"
export FACTORY_CONTRACT="$FACTORY_CONTRACT"

# Build the migrate message with the correct factory address
MIGRATE_MSG=$(
  jq -nc \
    --arg denom   "$ORO_DENOM" \
    --arg vesting "$VESTING_CONTRACT" \
    --arg factory "$FACTORY_CONTRACT" \
    '{
      oro_token: { native_token: { denom: $denom } },
      vesting_contract: $vesting,
      factory: $factory
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
