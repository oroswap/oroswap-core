#!/bin/bash
set -euo pipefail

# Usage:
#   ENV=testnet bash deploy.sh   → uses testnet_testing.env (zig-test-2)
#   ENV=mainnet bash deploy.sh   → uses mainnet.env (zigchain-1)
#
# Required env vars (set before running or export in your shell):
#   REVENUE_ROUTER_CODE_ID  — code ID after storing the wasm
#   DEST_RECEIVER           — final receiver address on the destination chain
#                             testnet: your osmo1... address
#                             mainnet: your Ethereum 0x... address

ENV=${ENV:-testnet}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ "$ENV" = "testnet" ]; then
    source "$SCRIPT_DIR/../../testnet_testing.env"
    USDC_DENOM=${USDC_DENOM:-"ibc/5260516290F7883EC893AADA09A6B8CEC790F2EEF3196F440037908749785BE8"}
    DEST_CHAIN_ID="osmo-test-5"
    DEST_USDC_DENOM="ibc/DE6792CF9E521F6AD6E9A4BDF6225C9571A3B74ACC0A529F92BC5122A39D2E58"
    IBC_CHANNEL=${IBC_CHANNEL:-"channel-44"}
elif [ "$ENV" = "mainnet" ]; then
    source "$SCRIPT_DIR/../../mainnet.env"
    USDC_DENOM=${USDC_DENOM:-"ibc/6490A7EAB61059BFC1CDDEB05917DD70BDF3A611654162A1A47DB930D40D8AF4"}
    DEST_CHAIN_ID="1"
    DEST_USDC_DENOM="0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    IBC_CHANNEL=${IBC_CHANNEL:-"channel-3"}
else
    echo "❌ Unknown ENV=$ENV. Use testnet or mainnet."; exit 1
fi

# ── Validate ──────────────────────────────────────────────────────────────────
REVENUE_ROUTER_CODE_ID=${REVENUE_ROUTER_CODE_ID:-0}
if [ "$REVENUE_ROUTER_CODE_ID" -eq 0 ]; then
    echo "❌ REVENUE_ROUTER_CODE_ID not set. Store the wasm first then export it."; exit 1
fi

# DEST_RECEIVER is the final address that receives USDC on the destination chain.
#   testnet: your Osmosis testnet address  (osmo1...)
#   mainnet: your Ethereum treasury address (0x...)
# This is NOT derived automatically — you must set it explicitly.
if [ -z "${DEST_RECEIVER:-}" ]; then
    echo "❌ DEST_RECEIVER not set."
    echo "   testnet: export DEST_RECEIVER=osmo1..."
    echo "   mainnet: export DEST_RECEIVER=0x..."
    exit 1
fi

ADMIN_ADDR=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

echo "🚀 Deploying Revenue Router ($ENV)"
echo "   admin        : $ADMIN_ADDR"
echo "   usdc_denom   : $USDC_DENOM"
echo "   dest_chain   : $DEST_CHAIN_ID"
echo "   ibc_channel  : $IBC_CHANNEL"
echo "   dest_receiver: $DEST_RECEIVER"
echo ""

# ── Instantiate ───────────────────────────────────────────────────────────────
MSG=$(cat <<EOF
{
  "owner": "$ADMIN_ADDR",
  "usdc_denom": "$USDC_DENOM",
  "dest_chain_id": "$DEST_CHAIN_ID",
  "dest_usdc_denom": "$DEST_USDC_DENOM",
  "dest_receiver": "$DEST_RECEIVER",
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
