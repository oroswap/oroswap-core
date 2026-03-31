#!/bin/bash
set -euo pipefail

# Usage:
#   ENV=testnet bash deploy.sh   → uses testnet_testing.env (zig-test-2)
#   ENV=mainnet bash deploy.sh   → uses mainnet.env (zigchain-1)
#
# Required env vars (set before running or export in your shell):
#   REVENUE_ROUTER_CODE_ID  — code ID after storing the wasm

ENV=${ENV:-testnet}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ "$ENV" = "testnet" ]; then
    source "$SCRIPT_DIR/../../testnet_testing.env"
    USDC_DENOM=${USDC_DENOM:-"ibc/5260516290F7883EC893AADA09A6B8CEC790F2EEF3196F440037908749785BE8"}
    IBC_CHANNEL=${IBC_CHANNEL:-"channel-44"}
elif [ "$ENV" = "mainnet" ]; then
    source "$SCRIPT_DIR/../../mainnet.env"
    USDC_DENOM=${USDC_DENOM:-"ibc/6490A7EAB61059BFC1CDDEB05917DD70BDF3A611654162A1A47DB930D40D8AF4"}
    IBC_CHANNEL=${IBC_CHANNEL:-"channel-3"}
else
    echo "❌ Unknown ENV=$ENV. Use testnet or mainnet."; exit 1
fi

# ── Validate ──────────────────────────────────────────────────────────────────
REVENUE_ROUTER_CODE_ID=${REVENUE_ROUTER_CODE_ID:-0}
if [ "$REVENUE_ROUTER_CODE_ID" -eq 0 ]; then
    echo "❌ REVENUE_ROUTER_CODE_ID not set. Store the wasm first then export it."; exit 1
fi



ADMIN_ADDR=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

echo "🚀 Deploying Revenue Router ($ENV)"
echo "   admin        : $ADMIN_ADDR"
echo "   usdc_denom   : $USDC_DENOM"
echo "   ibc_channel  : $IBC_CHANNEL"
echo ""

# ── Instantiate ───────────────────────────────────────────────────────────────
MSG=$(cat <<EOF
{
  "owner": "$ADMIN_ADDR",
  "usdc_denom": "$USDC_DENOM",
  "ibc_channel": "$IBC_CHANNEL",
  "ibc_timeout_secs": 600
}
EOF
)

$BINARY tx wasm instantiate "$REVENUE_ROUTER_CODE_ID" "$MSG" \
  --label "oroswap-revenue-router" \
  --admin "$ADMIN_ADDR" \
  --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json | jq

echo "⏳ Waiting for transaction..."
sleep "$SLEEP_TIME"

CONTRACT_ADDR=$($BINARY query wasm list-contract-by-code "$REVENUE_ROUTER_CODE_ID" \
  --node "$RPC_URL" --output json | jq -r '.contracts[-1]')

echo "✅ Revenue Router: $CONTRACT_ADDR"
echo ""
echo "📝 Add to your .env:"
echo "export REVENUE_ROUTER_CONTRACT=\"$CONTRACT_ADDR\""
