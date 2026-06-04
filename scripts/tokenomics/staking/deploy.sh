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

# ─── Tunables ───────────────────────────────────────────────────────────────
# Unbonding window in seconds (default: 7 days).
UNBONDING_PERIOD=${UNBONDING_PERIOD:-604800}

# Per-operation dust floor (Uint128 as a JSON string). Default 1 ORO at
# 6 decimals. Enforced on every Enter and every partial Leave; full exits
# bypass the floor so users can always unwind sub-minimum dust.
MINIMUM_STAKE_AMOUNT=${MINIMUM_STAKE_AMOUNT:-1000000}    # 1 ORO at 6 decimals

# The contract is a 1:1 ORO stake registry — no bootstrap principal, no
# reward reserve. Instantiate is funds-less.

# NOTE: Pause authorities are added post-deploy via the test.sh
# `add_pause_authorities` helper (matches factory/incentives operator pattern).
# `InstantiateMsg` does not accept a `pause_authorities` field.

echo "🚀 Instantiating Staking Contract..."
MSG=$(cat <<EOF
{
  "owner": "$ADMIN_ADDR",
  "oro_denom": "$ORO_DENOM",
  "unbonding_period": $UNBONDING_PERIOD,
  "minimum_stake_amount": "$MINIMUM_STAKE_AMOUNT"
}
EOF
)

# Broadcast instantiation
$BINARY tx wasm instantiate "$OROSWAP_STAKING_CODE_ID" "$MSG" \
  --label "oroswap-staking" \
  --admin "$ADMIN_ADDR" \
  --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json | jq

echo "⏳ Waiting for transaction..."
sleep "$SLEEP_TIME"

# Get the contract address
STAKING_ADDR=$($BINARY query wasm list-contract-by-code "$OROSWAP_STAKING_CODE_ID" \
  --node "$RPC_URL" --output json | jq -r '.contracts[-1]')

echo "✅ Staking Contract Address: $STAKING_ADDR"
echo "ℹ️  1:1 stake registry: STAKES[user] = ORO staked, tracked in contract storage."

echo "📝 Update your .env file with:"
echo "export STAKING_CONTRACT=\"$STAKING_ADDR\""
