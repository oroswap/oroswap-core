#!/bin/bash
set -euo pipefail

# Usage:
#   ENV=testnet bash deploy.sh   → uses testnet_testing.env (zig-test-2)
#   ENV=mainnet bash deploy.sh   → uses mainnet.env (zigchain-1)
#
# Required env vars (set before running or export in your shell):
#   REVENUE_ROUTER_CODE_ID — code ID after storing the wasm
#   FIRST_HOP_RECEIVER     — noble1... intermediary (run precompute.js first)
#   FORWARD_MEMO           — PFM/CCTP JSON memo  (run precompute.js first)
# Optional:
#   MAX_BRIDGE_PER_TX      — micro-USDC cap per BridgeUsdc call (uncapped if empty)
#   IBC_TIMEOUT_SECS       — packet timeout (default 600)

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

# Also pull FIRST_HOP_RECEIVER + FORWARD_MEMO (and MAX_BRIDGE_PER_TX) from the
# per-route .env if they live there.
if [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1090
    source "$SCRIPT_DIR/.env"
    set +a
elif [ -f "$SCRIPT_DIR/.env.$ENV" ]; then
    set -a
    # shellcheck disable=SC1090
    source "$SCRIPT_DIR/.env.$ENV"
    set +a
fi

IBC_TIMEOUT_SECS=${IBC_TIMEOUT_SECS:-600}

# ── Validate ──────────────────────────────────────────────────────────────────
REVENUE_ROUTER_CODE_ID=${REVENUE_ROUTER_CODE_ID:-0}
if [ "$REVENUE_ROUTER_CODE_ID" -eq 0 ]; then
    echo "❌ REVENUE_ROUTER_CODE_ID not set. Store the wasm first then export it."; exit 1
fi
if [ -z "${FIRST_HOP_RECEIVER:-}" ]; then
    echo "❌ FIRST_HOP_RECEIVER not set. Run: node precompute.js >> .env.$ENV"; exit 1
fi
if [ -z "${FORWARD_MEMO:-}" ]; then
    echo "❌ FORWARD_MEMO not set. Run: node precompute.js >> .env.$ENV"; exit 1
fi

ADMIN_ADDR=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

echo "🚀 Deploying Revenue Router ($ENV)"
echo "   admin              : $ADMIN_ADDR"
echo "   usdc_denom         : $USDC_DENOM"
echo "   ibc_channel        : $IBC_CHANNEL"
echo "   first_hop_receiver : $FIRST_HOP_RECEIVER"
echo "   forward_memo       : $(echo "$FORWARD_MEMO" | head -c 120)..."
echo "   max_bridge_per_tx  : ${MAX_BRIDGE_PER_TX:-<uncapped>}"
echo ""

# ── Build the InstantiateMsg safely with jq (avoids double-escaping the memo JSON)
if [ -n "${MAX_BRIDGE_PER_TX:-}" ]; then
    MSG=$(jq -n \
      --arg owner              "$ADMIN_ADDR" \
      --arg usdc_denom         "$USDC_DENOM" \
      --arg ibc_channel        "$IBC_CHANNEL" \
      --argjson ibc_timeout_secs "$IBC_TIMEOUT_SECS" \
      --arg first_hop_receiver "$FIRST_HOP_RECEIVER" \
      --arg forward_memo       "$FORWARD_MEMO" \
      --arg max_bridge_per_tx  "$MAX_BRIDGE_PER_TX" \
      '{
         owner: $owner,
         usdc_denom: $usdc_denom,
         ibc_channel: $ibc_channel,
         ibc_timeout_secs: $ibc_timeout_secs,
         first_hop_receiver: $first_hop_receiver,
         forward_memo: $forward_memo,
         max_bridge_per_tx: $max_bridge_per_tx
       }')
else
    MSG=$(jq -n \
      --arg owner              "$ADMIN_ADDR" \
      --arg usdc_denom         "$USDC_DENOM" \
      --arg ibc_channel        "$IBC_CHANNEL" \
      --argjson ibc_timeout_secs "$IBC_TIMEOUT_SECS" \
      --arg first_hop_receiver "$FIRST_HOP_RECEIVER" \
      --arg forward_memo       "$FORWARD_MEMO" \
      '{
         owner: $owner,
         usdc_denom: $usdc_denom,
         ibc_channel: $ibc_channel,
         ibc_timeout_secs: $ibc_timeout_secs,
         first_hop_receiver: $first_hop_receiver,
         forward_memo: $forward_memo
       }')
fi

echo "📝 Instantiate msg:"
echo "$MSG" | jq .
echo ""

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
echo "📋 Eyeball the destination policy that's now live on chain:"
$BINARY query wasm contract-state smart "$CONTRACT_ADDR" '{"preview_bridge":{}}' \
  --node "$RPC_URL" --output json | jq .
echo ""
echo "📝 Add to your .env:"
echo "CONTRACT_ADDRESS=\"$CONTRACT_ADDR\""
