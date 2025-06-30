#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://testnet-rpc.zigchain.com"
CHAIN_ID="zig-test-2"
KEY_NAME="testnet-admin"
KEYRING_BACKEND="file"
# Replace the following with your actual testnet pair contract address
PAIR_CONTRACT="zig1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
GAS_ADJUSTMENT="1.3"
GAS_PRICES_NATIVE="0.25uzig"
FEES_NATIVE="2000uzig"
BLOCK_TIME=5

# ─── Function: Add liquidity to XYK pair ─────────────────────────────────────
# Usage: add_liquidity <amount1> <denom1> <amount2> <denom2> [slippage]
add_liquidity() {
  local amt1="$1" denom1="$2" amt2="$3" denom2="$4" slippage="${5:-"0.005"}"
  echo "▶️ Adding liquidity: $amt1$denom1 + $amt2$denom2 (slippage: $slippage)"
  $BINARY tx wasm execute "$PAIR_CONTRACT" \
    '{
      "provide_liquidity": {
        "assets": [
          {"info": {"native_token": {"denom": "'"$denom1"'"}}, "amount": "'"$amt1"'"},
          {"info": {"native_token": {"denom": "'"$denom2"'"}}, "amount": "'"$amt2"'"}
        ],
        "slippage_tolerance": "'"$slippage"'"
      }
    }' \
    --amount "${amt1}${denom1},${amt2}${denom2}" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME
}

# ─── Function: Remove liquidity from XYK pair ─────────────────────────────────
# Usage: remove_liquidity <share_amount>
remove_liquidity() {
  local share="$1"
  echo "▶️ Removing liquidity: share_amount=$share"
  $BINARY tx wasm execute "$PAIR_CONTRACT" \
    '{"withdraw_liquidity": {"share": "'"$share"'"}}' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME
}

# ─── Function: Query pair state ──────────────────────────────────────────────
# Usage: query_pair
query_pair() {
  echo "🔍 Querying pair info..."
  $BINARY query wasm contract-state smart "$PAIR_CONTRACT" '{"pair":{}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── Function: Query pool reserves ───────────────────────────────────────────
# Usage: query_pool
query_pool() {
  echo "🔍 Querying pool state..."
  $BINARY query wasm contract-state smart "$PAIR_CONTRACT" '{"pool":{}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── Function: Simulate swap ─────────────────────────────────────────────────
# Usage: query_swap <amount> <denom>
query_swap() {
  local amt="$1" denom="$2"
  echo "🔍 Simulating swap: offer $amt$denom"
  $BINARY query wasm contract-state smart "$PAIR_CONTRACT" \
    '{
      "simulation": {
        "offer_asset": {
          "info": {"native_token": {"denom": "'"$denom"'"}},
          "amount": "'"$amt"'"
        }
      }
    }' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── Function: Execute swap ───────────────────────────────────────────────────
# Usage: swap <denom> <amount> [belief_price] [max_spread] [to]
swap() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 swap <denom> <amount> [belief_price] [max_spread] [to]"; exit 1; }
  local denom="$1" amount="$2" belief_price="${3:-}" max_spread="${4:-}" to_addr="${5:-}"
  echo "▶️ Swapping $amount$denom"
  BASE=$(jq -nc --arg d "$denom" --arg a "$amount" '{ offer_asset: { info: { native_token: { denom: $d }}, amount: $a } }')
  SWAP=$(echo "$BASE" | jq \
    --arg bp "$belief_price" --arg ms "$max_spread" --arg to "$to_addr" '
      . as $m |
      ($m + (if $bp != "" then { belief_price: $bp } else {} end)
           + (if $ms != "" then { max_spread: $ms } else {} end)
           + (if $to != "" then { to: $to } else {} end))
      | { swap: . }
    ')
  $BINARY tx wasm execute "$PAIR_CONTRACT" "$SWAP" \
    --amount "${amount}${denom}" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME
}

# If script is called without args, show usage
if [[ $# -eq 0 ]]; then
  echo "Usage:"
  echo "  $0 add_liquidity <amt1> <denom1> <amt2> <denom2> [slippage]"
  echo "  $0 remove_liquidity <share_amount>"
  echo "  $0 query_pair"
  echo "  $0 query_pool"
  echo "  $0 query_swap <amount> <denom>"
  echo "  $0 swap <denom> <amount> [belief_price] [max_spread] [to]"
  exit 0
fi

# Dispatch based on first argument
case "$1" in
  add_liquidity) shift; add_liquidity "$@" ;;
  remove_liquidity) shift; remove_liquidity "$@" ;;
  query_pair) query_pair ;;
  query_pool) query_pool ;;
  query_swap) shift; query_swap "$@" ;;
  swap) shift; swap "$@" ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
