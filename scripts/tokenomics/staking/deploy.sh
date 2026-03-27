#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
# Source environment variables if available
if [ -f "$(dirname "$0")/../../devnet_testing.env" ]; then
    source "$(dirname "$0")/../../devnet_testing.env"
else
    echo "⚠️ devnet_testing.env not found, using defaults"
    exit 1
fi

# Contract specific (Update these after storing code)
OROSWAP_STAKING_CODE_ID=${OROSWAP_STAKING_CODE_ID:-0}

if [ "$OROSWAP_STAKING_CODE_ID" -eq 0 ]; then
    echo "❌ OROSWAP_STAKING_CODE_ID is not set. Please store the code first or set the variable."
    exit 1
fi


# ─── Deployment ─────────────────────────────────────────────────────────────
ADMIN_ADDR=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

echo "🚀 Instantiating Staking Contract..."
MSG=$(cat <<EOF
{
  "deposit_token_denom": "$ORO_DENOM",
  "unbonding_period": 604800,
  "bootstrap_amount": "1000",
  "xoro_decimals": 6,
  "xoro_uri": ""
}
EOF
)

# Broadcast instantiation
$BINARY tx wasm instantiate "$OROSWAP_STAKING_CODE_ID" "$MSG" \
  --label "oroswap-staking" \
  --admin "$ADMIN_ADDR" \
  --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  --amount "1000$ORO_DENOM,2000uzig" \
  --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json | jq

echo "⏳ Waiting for transaction..."
sleep "$SLEEP_TIME"

# Get the contract address
STAKING_ADDR=$($BINARY query wasm list-contract-by-code "$OROSWAP_STAKING_CODE_ID" \
  --node "$RPC_URL" --output json | jq -r '.contracts[-1]')

echo "✅ Staking Contract Address: $STAKING_ADDR"

echo "📝 Update your .env file with:"
echo "export STAKING_CONTRACT=\"$STAKING_ADDR\""
