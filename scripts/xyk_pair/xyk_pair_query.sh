#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://devnet-rpc.zigchain.com"
CHAIN_ID="zig-devnet-1"
KEY_NAME="devnet-key"
KEYRING_BACKEND="test"
PAIR_CONTRACT="zig1hgu5z4nwcxngsm9tqfwj84k8n73wfddfrvchg39lrm44zahn0fdq4pt0ag"
GAS_ADJUSTMENT="1.3"
GAS_PRICES_NATIVE="0.25uzig"
FEES_NATIVE="2000uzig"
BLOCK_TIME=5

# ─── Function: Add liquidity (native + native) ──────────────────────────────
# Usage: add_liquidity <amt1> <denom1> <amt2> <denom2> [slippage] [auto_stake]
add_liquidity() {
  local amt1="$1" denom1="$2" amt2="$3" denom2="$4" slippage="${5:-"0.005"}" auto_stake="${6:-"true"}"
  echo "▶️ Adding liquidity: $amt1$denom1 + $amt2$denom2 (slippage: $slippage, auto_stake: $auto_stake)"
  $BINARY tx wasm execute "$PAIR_CONTRACT" \
    '{
      "provide_liquidity": {
        "assets": [
          {"info": {"native_token": {"denom": "'"$denom1"'" }}, "amount": "'"$amt1"'"},
          {"info": {"native_token": {"denom": "'"$denom2"'" }}, "amount": "'"$amt2"'"}
        ],
        "slippage_tolerance": "'"$slippage"'",
        "auto_stake": '"$auto_stake"'
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

# ─── Function: Add liquidity (CW20 + native) ───────────────────────────────
# Usage: add_liquidity_cw20 <cw20_contract> <cw20_amt> <nat_amt> <nat_denom> [slippage]
add_liquidity_cw20() {
  [ "$#" -ge 4 ] || { echo "Usage: $0 add_liquidity_cw20 <cw20_contract> <cw20_amt> <nat_amt> <nat_denom> [slippage] [to_address]"; exit 1; }
  local cw20_contract="$1"
  local cw20_amt="$2"
  local nat_amt="$3"
  local nat_denom="$4"
  local slippage="${5:-"0.005"}"
  local to_addr="${6:-}"   # optional recipient for liquidity tokens

  echo "▶️ Adding liquidity (CW-20 + Native):"
  echo "   • CW-20 contract: $cw20_contract"
  echo "   • CW-20 amount:   $cw20_amt"
  echo "   • Native amount:  $nat_amt$nat_denom"
  echo "   • Slippage:       $slippage"
  [[ -n "$to_addr" ]] && echo "   • Recipient:      $to_addr"

  echo "DEBUG: Approving CW20 token..."
  # First approve the pair contract to spend the CW20 token
  $BINARY tx wasm execute "$cw20_contract" \
    '{ "increase_allowance": { "spender": "'"$PAIR_CONTRACT"'", "amount": "'"$cw20_amt"'", "expires": null } }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME

  echo "DEBUG: Providing liquidity..."
  # Then provide liquidity directly to the pair contract
  $BINARY tx wasm execute "$PAIR_CONTRACT" \
    '{
      "provide_liquidity": {
        "assets": [
          { "info": { "token": { "contract_addr": "'"$cw20_contract"'" } }, "amount": "'"$cw20_amt"'" },
          { "info": { "native_token": { "denom": "'"$nat_denom"'" } }, "amount": "'"$nat_amt"'" }
        ],
        "slippage_tolerance": "'"$slippage"'"'$( [[ -n "$to_addr" ]] && printf ', "receiver": "%s"' "$to_addr" )'
      }
    }' \
    --amount "${nat_amt}${nat_denom}" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME
}

# ─── Function: Remove liquidity ─────────────────────────────────────────────
# Usage: remove_liquidity <LP_DENOM> <share_amount>
remove_liquidity() {
  LP_DENOM=$1; AMT=$2
    echo "Withdraw $AMT LP tokens of denom $LP_DENOM"
    PAYLOAD=$(jq -nc '{ withdraw_liquidity: {} }')
    zigchaind tx wasm execute "$PAIR_CONTRACT" "$PAYLOAD" \
      --amount "${AMT}${LP_DENOM}" \
      --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
     -y
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME
}


# ─── Function: Query pair info ──────────────────────────────────────────────
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

# ─── Function: Query asset balance at specific block height ──────────────────
# Usage: query_asset_balance_at <denom> <block_height>
query_asset_balance_at() {
  [ "$#" -eq 2 ] || { echo "Usage: $0 query_asset_balance_at <denom> <block_height>"; echo "Example: $0 query_asset_balance_at uzig 2299000"; exit 1; }
  local denom="$1"
  local block_height="$2"
  
  echo "🔍 Querying asset balance at block height $block_height for $denom..."
  $BINARY query wasm contract-state smart "$PAIR_CONTRACT" \
    '{
      "asset_balance_at": {
        "asset_info": {
          "native_token": {
            "denom": "'"$denom"'"
          }
        },
        "block_height": "'"$block_height"'"
      }
    }' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── Function: Query cumulative price ───────────────────────────────────────
# Usage: query_cumulative_price
query_cumulative_price() {
  echo "🔍 Querying cumulative price..."
  $BINARY query wasm contract-state smart "$PAIR_CONTRACT" \
    '{
      "cumulative_prices": {}
    }' \
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
          "info": { "native_token": { "denom": "'"$denom"'" } },
          "amount": "'"$amt"'"
        }
      }
    }' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── Function: Execute swap ───────────────────────────────────────────────────
# Usage: swap <denom> <amount> [max_spread] [to]
swap() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 swap <denom> <amount> [max_spread] [to]"; exit 1; }
  local denom="$1" amount="$2" max_spread="${3:-"0.005"}" to_addr="${4:-}"
  echo "▶️ Swapping $amount$denom with max_spread=$max_spread (to=${to_addr:-not set})"

  # 1) Derive belief_price via simulation
  local sim_json return_amount belief_price
  sim_json=$($BINARY query wasm contract-state smart "$PAIR_CONTRACT" \
    '{ "simulation": { "offer_asset": { "info": { "native_token": { "denom": "'"$denom"'" } }, "amount": "'"$amount"'" } } }' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json)
  return_amount=$(echo "$sim_json" | jq -r '.data.return_amount')
  belief_price=$(echo "scale=18; $amount / $return_amount" | bc -l | sed 's/^\./0./')
  echo "👉 Derived belief_price=$belief_price"

  # 2) Build swap message
  local BASE SWAP
  BASE=$(jq -nc --arg d "$denom" --arg a "$amount" '{ offer_asset: { info: { native_token: { denom: $d }}, amount: $a } }')
  SWAP=$(echo "$BASE" | jq --arg bp "$belief_price" --arg ms "$max_spread" --arg to "$to_addr" '
    . as $m |
    ($m + { belief_price: $bp, max_spread: $ms }
      + (if ($to | length) > 0 then { to: $to } else {} end))
    | { swap: . }
  ')

  # 3) Execute swap
  $BINARY tx wasm execute "$PAIR_CONTRACT" "$SWAP" \
    --amount "${amount}${denom}" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq
  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion..."
  sleep $BLOCK_TIME
}



swap_cw20() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 swap_cw20 <cw20_contract> <amount> [max_spread] [to_address]"; exit 1; }
  local cw20_contract="$1"
  local amount="$2"
  local max_spread="${3:-0.005}"
  local to_addr="${4:-}"

  echo "▶️ Swapping CW20 → native on pair $PAIR_CONTRACT:"
  echo "   • CW20 contract: $cw20_contract"
  echo "   • CW20 amount:   $amount"
  echo "   • Max spread:    $max_spread"
  [[ -n "$to_addr" ]] && echo "   • Recipient:     $to_addr"

  # 1) Query the pair's simulation endpoint to derive return_amount + belief_price
  local sim_json return_amount belief_price
  sim_json=$(
    $BINARY query wasm contract-state smart "$PAIR_CONTRACT" \
      '{"simulation":{"offer_asset":{"info":{"token":{"contract_addr":"'"$cw20_contract"'"}},"amount":"'"$amount"'"}}}' \
      --node "$RPC_URL" \
      --chain-id "$CHAIN_ID" \
      -o json
  )
  return_amount=$(echo "$sim_json" | jq -r '.data.return_amount')
  belief_price=$(echo "scale=18; $return_amount / $amount" | bc -l)
  echo "👉 Derived belief_price = $belief_price"

  # 2) Build the hook msg inline
  if [ -n "$to_addr" ]; then
    hook_json='{"swap":{"belief_price":"'"$belief_price"'","max_spread":"'"$max_spread"'","to":"'"$to_addr"'"}}'
  else
    hook_json='{"swap":{"belief_price":"'"$belief_price"'","max_spread":"'"$max_spread"'"}}'
  fi

  # 3) Base64‐encode the hook payload
  local hook_b64
  hook_b64=$(printf '%s' "$hook_json" | base64 | tr -d '\n')
  echo "DEBUG: CW20‐hook payload (first 20 chars base64): ${hook_b64:0:20}…"

  # 4) Send a single cw20.send→hook to the pair
  echo "▶️ Executing cw20.send→swap hook in one transaction…"
  $BINARY tx wasm execute "$cw20_contract" \
    '{"send":{"contract":"'"$PAIR_CONTRACT"'","amount":"'"$amount"'","msg":"'"$hook_b64"'"}}' \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" \
    --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES_NATIVE" \
    -y -o json | jq

  echo "⏳ Waiting $BLOCK_TIME seconds for block inclusion…"
  sleep "$BLOCK_TIME"
}




# ─── Function: add_liquidity, add_liquidity_cw20, remove_liquidity, query_pair, query_pool, query_swap …(not shown)───
# … (other functions remain unchanged) …

# ─── Dispatch block ─────────────────────────────────────────────────────────
if [[ $# -eq 0 ]]; then
  echo "Usage:"
  echo "  $0 add_liquidity <amt1> <denom1> <amt2> <denom2> [slippage] [auto_stake]"
  echo "  $0 add_liquidity_cw20 <cw20_contract> <cw20_amt> <nat_amt> <nat_denom> [slippage] [to]"
  echo "  $0 remove_liquidity <LPdenom> <amount>"
  echo "  $0 query_pair"
  echo "  $0 query_pool"
  echo "  $0 query_asset_balance_at <denom> <block_height>"
  echo "  $0 query_swap <amount> <denom>"
  echo "  $0 swap <denom> <amount> [max_spread] [to]"
  echo "  $0 swap_cw20 <cw20_contract> <amount> [max_spread] [to]"
  echo "  $0 query_cumulative_price"
  exit 0
fi

case "$1" in
  add_liquidity)          shift; add_liquidity "$@" ;;
  add_liquidity_cw20)     shift; add_liquidity_cw20 "$@" ;;
  remove_liquidity)       shift; remove_liquidity "$@" ;;
  query_pair)             query_pair ;;
  query_pool)             query_pool ;;
  query_asset_balance_at) shift; query_asset_balance_at "$@" ;;
  query_swap)             shift; query_swap "$@" ;;
  swap)                   shift; swap "$@" ;;
  swap_cw20)              shift; swap_cw20 "$@" ;;
  query_cumulative_price) query_cumulative_price ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac