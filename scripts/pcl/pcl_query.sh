#!/usr/bin/env bash
set -euo pipefail

# ─── Environment ────────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://devnet-rpc.zigchain.com"
CHAIN_ID="zig-devnet-1"
KEY_NAME="devnet-key"
KEYRING_BACKEND="test"
NODE="$RPC_URL"
KEY="$KEY_NAME"
FEES="2000uzig"

# ─── Pair contract to tweak ─────────────────────────────────────────────────
PAIR_CONTRACT="zig1g6x75tzgefppgq22e0qeamyrprmy3qgwtt76w56cvydwfcngud5qja9j4r"

# Helper: smart query
defq(){
  if [ "$1" = '{"config":{}}' ]; then
    zigchaind query wasm contract-state smart "$PAIR_CONTRACT" "$1" \
      --chain-id "$CHAIN_ID" --node "$NODE" -o json | jq '.data.params |= (if . != null then @base64d else . end)'
  else
    zigchaind query wasm contract-state smart "$PAIR_CONTRACT" "$1" \
      --chain-id "$CHAIN_ID" --node "$NODE" -o json | jq .
  fi
}

usage(){
  grep '^#' "$0" | sed 's/#//g'
  exit 1
}

[ "$#" -ge 1 ] || usage
COMMAND=$1; shift

case "$COMMAND" in
  provide)
    [ "$#" -eq 4 ] || usage
    DEN1=$1; AMT1=$2; DEN2=$3; AMT2=$4
    PAYLOAD=$(jq -nc \
      --arg d1 "$DEN1" \
      --arg d2 "$DEN2" \
      --arg a1 "$AMT1" \
      --arg a2 "$AMT2" \
      '{
        provide_liquidity: {
          assets: [
            { info: { native_token: { denom: $d1 }}, amount: $a1 },
            { info: { native_token: { denom: $d2 }}, amount: $a2 }
          ],
          auto_stake: false,
          receiver: null,
          slippage_tolerance: "0.005"
        }
      }')
    echo "Provide liquidity: $AMT1 $DEN1 and $AMT2 $DEN2 (tolerance 1%)"
    zigchaind tx wasm execute "$PAIR_CONTRACT" "$PAYLOAD" \
      --amount "${AMT1}${DEN1},${AMT2}${DEN2}" \
      --from "$KEY" --chain-id "$CHAIN_ID" --node "$NODE" \
      --fees "$FEES" --gas auto --gas-adjustment 1.3 -y
    ;;

  withdraw)
    [ "$#" -eq 2 ] || usage
    LP_DENOM=$1; AMT=$2
    echo "Withdraw $AMT LP tokens of denom $LP_DENOM"
    PAYLOAD=$(jq -nc '{ withdraw_liquidity: {} }')
    zigchaind tx wasm execute "$PAIR_CONTRACT" "$PAYLOAD" \
      --amount "${AMT}${LP_DENOM}" \
      --from "$KEY" --chain-id "$CHAIN_ID" --node "$NODE" \
      --fees "$FEES" --gas auto --gas-adjustment 1.3 -y
    ;;

  swap)
    [ "$#" -ge 2 ] || usage
    DEN=$1; AMT=$2; BELIEF=${3:-}; MAX_SPREAD=${4:-}; TO=${5:-}
    # Build base swap message
    BASE=$(jq -nc --arg d "$DEN" --arg a "$AMT" '{ offer_asset: { info: { native_token: { denom: $d }}, amount: $a } }')
    # Add optional fields as strings
    SWAP=$(echo "$BASE" | jq \
      --arg bp "$BELIEF" --arg ms "$MAX_SPREAD" --arg to "$TO" '
      . as $m |
      ($m + (if $bp != "" then { belief_price: $bp } else {} end)
           + (if $ms != "" then { max_spread: $ms } else {} end)
           + (if $to != "" then { to: $to } else {} end))
      | { swap: . }
    ')
    echo "Swap $AMT $DEN"
    zigchaind tx wasm execute "$PAIR_CONTRACT" "$SWAP" \
      --amount "${AMT}${DEN}" \
      --from "$KEY" --chain-id "$CHAIN_ID" --node "$NODE" \
      --fees "$FEES" --gas auto --gas-adjustment 1.3 -y
    ;;

  quote|simulate)
    [ "$#" -eq 2 ] || usage
    DEN=$1; AMT=$2
    echo "Quote for $AMT $DEN"
    defq "{ \"simulation\":{\"offer_asset\":{\"info\":{\"native_token\":{\"denom\":\"$DEN\"}},\"amount\":\"$AMT\"}} }"
    ;;

  reverse)
    [ "$#" -eq 2 ] || usage
    DEN=$1; AMT=$2
    echo "Reverse quote for $AMT $DEN"
    defq "{ \"reverse_simulation\":{\"ask_asset\":{\"info\":{\"native_token\":{\"denom\":\"$DEN\"}},\"amount\":\"$AMT\"}} }"
    ;;

  update-config)
    [ "$#" -eq 1 ] || usage
    B64=$1
    MSG=$(jq -nc --arg ip "$B64" '{ update_config: { params: $ip }}')
    echo "Update config params"
    zigchaind tx wasm execute "$PAIR_CONTRACT" "$MSG" \
      --from "$KEY" --chain-id "$CHAIN_ID" --node "$NODE" \
      --fees "$FEES" --gas auto --gas-adjustment 1.3 -y
    ;;

  pair|pool|config|amp-gamma|cumulative-prices|compute-d|lp-price)
    case "$COMMAND" in
      pair)              ARG='{"pair":{}}' ;;
      pool)              ARG='{"pool":{}}' ;;
      config)            ARG='{"config":{}}' ;;
      amp-gamma)         ARG='{"amp_gamma":{}}' ;;
      cumulative-prices) ARG='{"cumulative_prices":{}}' ;;
      compute-d)         ARG='{"compute_d":{}}' ;;
      lp-price)          ARG='{"lp_price":{}}' ;;
    esac
    defq "$ARG"
    ;;

  simulate-provide)
  # require exactly 4 args now
  if [ "$#" -ne 4 ]; then
    echo "Usage: $0 simulate-provide <denom1> <amount1> <denom2> <amount2>"
    exit 1
  fi

  DENOM1="$1"
  AMOUNT1="$2"
  DENOM2="$3"
  AMOUNT2="$4"

  # Construct the payload for simulation
  PAYLOAD=$(jq -n \
    --arg denom1 "$DENOM1" \
    --arg amount1 "$AMOUNT1" \
    --arg denom2 "$DENOM2" \
    --arg amount2 "$AMOUNT2" \
    '{
      "simulate_provide": {
        "assets": [
          {
            "info": {"native_token": {"denom": $denom1}},
            "amount": $amount1
          },
          {
            "info": {"native_token": {"denom": $denom2}},
            "amount": $amount2
          }
        ],
        "slippage_tolerance": "0.01"
      }
    }')

  # Execute the simulation query
  zigchaind query wasm contract-state smart "$PAIR_CONTRACT" "$PAYLOAD" \
    --output json --node "$NODE"
  ;;

  simulate-withdraw)
  # require exactly 1 arg
  if [ "$#" -ne 1 ]; then
    echo "Usage: $0 simulate-withdraw <lp_amount>"
    exit 1
  fi

  LP_AMOUNT="$1"

  # Construct the payload for simulation
  PAYLOAD=$(jq -n \
    --arg lp_amount "$LP_AMOUNT" \
    '{
      "simulate_withdraw": {
        "lp_amount": $lp_amount
      }
    }')

  echo "Simulate withdraw: $LP_AMOUNT LP tokens"
  # Execute the simulation query
  zigchaind query wasm contract-state smart "$PAIR_CONTRACT" "$PAYLOAD" \
    --output json --node "$NODE"
  ;;

  *)
    usage
    ;;
esac
