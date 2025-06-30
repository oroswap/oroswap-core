#!/usr/bin/env bash
set -euo pipefail

# Load environment variables
source ../devnet.env

# Configuration
BINARY="$BINARY"
RPC_URL="$RPC_URL"
CHAIN_ID="$CHAIN_ID"
KEY_NAME="$KEY_NAME"
KEYRING_BACKEND="$KEYRING_BACKEND"
FACTORY="$FACTORY_CONTRACT"

# Default pair contract address (from your successful transaction)
DEFAULT_PAIR_ADDR="zig1hqhd3fcnplvdgp3g3qcnhqczxe8mhk6vpaw0eday6wlz62achf3qa304e2"

# Transaction settings
TX_FEES="$FEES"
GAS_ADJUSTMENT="$GAS_ADJUSTMENT"

# ─── Functions ──────────────────────────────────────────────────────────────

# Query all pairs from factory
# Usage: query_all_pairs
query_all_pairs() {
  echo "🔍 Querying all pairs from factory..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs": {"limit": 100}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query stable pairs from factory
# Usage: query_stable_pairs
query_stable_pairs() {
  echo "🔍 Querying stable pairs from factory..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs": {"pair_type": {"stable": {}}, "limit": 100}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query specific pair by address
# Usage: query_pair [pair_contract_addr]
query_pair() {
  local pair_addr="${1:-$DEFAULT_PAIR_ADDR}"
  echo "🔍 Querying pair: $pair_addr"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    '{"pair": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query pair pool info
# Usage: query_pool [pair_contract_addr]
query_pool() {
  local pair_addr="${1:-$DEFAULT_PAIR_ADDR}"
  echo "🔍 Querying pool info: $pair_addr"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    '{"pool": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query pair configuration
# Usage: query_config [pair_contract_addr]
query_config() {
  local pair_addr="${1:-$DEFAULT_PAIR_ADDR}"
  echo "🔍 Querying pair config: $pair_addr"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    '{"config": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query swap simulation
# Usage: query_simulation <offer_amount> <offer_denom> [max_spread] [pair_contract_addr]
query_simulation() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 query_simulation <offer_amount> <offer_denom> [max_spread] [pair_contract_addr]"; echo "Example: $0 query_simulation 100 uzig 0.01"; exit 1; }
  
  local offer_amount="$1"
  local offer_denom="$2"
  local max_spread="${3:-}"
  local pair_addr="${4:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔍 Querying swap simulation: $pair_addr"
  echo "Offer: $offer_amount $offer_denom"
  [ -n "$max_spread" ] && echo "Max spread: $max_spread"
  
  local offer_asset="{\"info\": {\"native_token\": {\"denom\": \"$offer_denom\"}}, \"amount\": \"$offer_amount\"}"
  
  local query_msg="{\"simulation\": {\"offer_asset\": $offer_asset}}"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    "$query_msg" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query reverse swap simulation
# Usage: query_reverse_simulation <ask_asset_json> [offer_asset_info_json] [pair_contract_addr]
query_reverse_simulation() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 query_reverse_simulation <ask_asset_json> [offer_asset_info_json] [pair_contract_addr]"; echo "Example: $0 query_reverse_simulation '{\"info\": {\"native_token\": {\"denom\": \"uzig\"}}, \"amount\": \"1000000\"}'"; exit 1; }
  
  local ask_asset="$1"
  local offer_asset_info="${2:-null}"
  local pair_addr="${3:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔍 Querying reverse swap simulation: $pair_addr"
  echo "Ask asset: $ask_asset"
  [ "$offer_asset_info" != "null" ] && echo "Offer asset info: $offer_asset_info"
  
  local query_msg
  if [ "$offer_asset_info" = "null" ]; then
    query_msg="{\"reverse_simulation\": {\"ask_asset\": $ask_asset}}"
  else
    query_msg="{\"reverse_simulation\": {\"ask_asset\": $ask_asset, \"offer_asset_info\": $offer_asset_info}}"
  fi
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    "$query_msg" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query share calculation
# Usage: query_share <lp_amount> [pair_contract_addr]
query_share() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 query_share <lp_amount> [pair_contract_addr]"; echo "Example: $0 query_share 1000000"; exit 1; }
  
  local lp_amount="$1"
  local pair_addr="${2:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔍 Querying share calculation: $pair_addr (LP amount: $lp_amount)"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    "{\"share\": {\"amount\": \"$lp_amount\"}}" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query cumulative prices
# Usage: query_cumulative_prices [pair_contract_addr]
query_cumulative_prices() {
  local pair_addr="${1:-$DEFAULT_PAIR_ADDR}"
  echo "🔍 Querying cumulative prices: $pair_addr"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    '{"cumulative_prices": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query observations (for oracle data)
# Usage: query_observe <seconds_ago> [pair_contract_addr]
query_observe() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 query_observe <seconds_ago> [pair_contract_addr]"; echo "Example: $0 query_observe 3600"; exit 1; }
  
  local seconds_ago="$1"
  local pair_addr="${2:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔍 Querying observations: $pair_addr (seconds ago: $seconds_ago)"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    "{\"observe\": {\"seconds_ago\": $seconds_ago}}" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query compute D (stable pool specific)
# Usage: query_compute_d [pair_contract_addr]
query_compute_d() {
  local pair_addr="${1:-$DEFAULT_PAIR_ADDR}"
  echo "🔍 Querying compute D: $pair_addr"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    '{"query_compute_d": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query simulate provide liquidity
# Usage: query_simulate_provide <assets_json> [pair_contract_addr]
query_simulate_provide() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 query_simulate_provide <assets_json> [pair_contract_addr]"; echo "Example: $0 query_simulate_provide '[{\"info\": {\"native_token\": {\"denom\": \"uzig\"}}, \"amount\": \"1000000\"}, {\"info\": {\"native_token\": {\"denom\": \"coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.stable101\"}}, \"amount\": \"1000000\"}]'"; exit 1; }
  
  local assets="$1"
  local pair_addr="${2:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔍 Querying simulate provide: $pair_addr"
  echo "Assets: $assets"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    "{\"simulate_provide\": {\"assets\": $assets}}" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query simulate withdraw liquidity
# Usage: query_simulate_withdraw <lp_amount> [pair_contract_addr]
query_simulate_withdraw() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 query_simulate_withdraw <lp_amount> [pair_contract_addr]"; exit 1; }
  
  local lp_amount="$1"
  local pair_addr="${2:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔍 Querying simulate withdraw: $pair_addr (LP amount: $lp_amount)"
  
  $BINARY query wasm contract-state smart "$pair_addr" \
    "{\"simulate_withdraw\": {\"lp_amount\": \"$lp_amount\"}}" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query factory configuration
# Usage: query_factory_config
query_factory_config() {
  echo "🔍 Querying factory configuration..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"config": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query factory fee info for stable pairs
# Usage: query_factory_fee_info
query_factory_fee_info() {
  echo "🔍 Querying factory fee info for stable pairs..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"fee_info": {"pair_type": {"stable": {}}}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query pair by assets
# Usage: query_pair_by_assets <asset1_json> <asset2_json>
query_pair_by_assets() {
  [ "$#" -eq 2 ] || { echo "Usage: $0 query_pair_by_assets <asset1_json> <asset2_json>"; echo "Example: $0 query_pair_by_assets '{\"native_token\": {\"denom\": \"uzig\"}}' '{\"native_token\": {\"denom\": \"coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.stable101\"}}'"; exit 1; }
  
  local asset1="$1"
  local asset2="$2"
  
  echo "🔍 Querying pair by assets..."
  echo "Asset 1: $asset1"
  echo "Asset 2: $asset2"
  
  $BINARY query wasm contract-state smart "$FACTORY" \
    "{\"pair\": {\"asset_infos\": [$asset1, $asset2]}}" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── Liquidity Functions ────────────────────────────────────────────────────

# Provide liquidity to stable pair
# Usage: provide_liquidity <amount1> <denom1> <amount2> <denom2> [pair_contract_addr] [receiver]
provide_liquidity() {
  [ "$#" -ge 4 ] || { echo "Usage: $0 provide_liquidity <amount1> <denom1> <amount2> <denom2> [pair_contract_addr] [receiver]"; echo "Example: $0 provide_liquidity 10000 coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.stable101 10000 uzig"; exit 1; }
  
  local amount1="$1"
  local denom1="$2"
  local amount2="$3"
  local denom2="$4"
  local pair_addr="${5:-$DEFAULT_PAIR_ADDR}"
  local receiver="${6:-}"
  
  echo "💧 Providing liquidity to pair: $pair_addr"
  echo "Asset 1: $amount1 $denom1"
  echo "Asset 2: $amount2 $denom2"
  [ -n "$receiver" ] && echo "Receiver: $receiver"
  
  # Build the assets array
  local assets_json="[
    {\"info\": {\"native_token\": {\"denom\": \"$denom1\"}}, \"amount\": \"$amount1\"},
    {\"info\": {\"native_token\": {\"denom\": \"$denom2\"}}, \"amount\": \"$amount2\"}
  ]"
  
  # Build the message - only include receiver if specified
  local msg_json
  if [ -n "$receiver" ]; then
    msg_json="{
      \"provide_liquidity\": {
        \"assets\": $assets_json,
        \"auto_stake\": false,
        \"receiver\": \"$receiver\"
      }
    }"
  else
    msg_json="{
      \"provide_liquidity\": {
        \"assets\": $assets_json,
        \"auto_stake\": false
      }
    }"
  fi
  
  # Calculate total funds needed
  local total_funds="$amount1$denom1,$amount2$denom2"
  
  echo "📤 Executing provide_liquidity transaction..."
  
  $BINARY tx wasm execute "$pair_addr" \
    "$msg_json" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    --amount "$total_funds" \
    -y -o json | jq
}

# Remove liquidity from stable pair
# Usage: remove_liquidity <lp_amount> [pair_contract_addr]
remove_liquidity() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 remove_liquidity <lp_amount> [pair_contract_addr]"; echo "Example: $0 remove_liquidity 1000000"; exit 1; }
  
  local lp_amount="$1"
  local pair_addr="${2:-$DEFAULT_PAIR_ADDR}"
  
  echo "💧 Removing liquidity from pair: $pair_addr"
  echo "LP amount: $lp_amount"
  
  # Get LP token denom from pair query
  local lp_denom
  lp_denom=$($BINARY query wasm contract-state smart "$pair_addr" \
    '{"pair": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq -r '.data.liquidity_token')
  
  if [ "$lp_denom" = "null" ] || [ -z "$lp_denom" ]; then
    echo "❌ Error: Could not get LP token denom"
    exit 1
  fi
  
  echo "LP token denom: $lp_denom"
  
  # Build the message
  local msg_json="{
    \"withdraw_liquidity\": {
      \"assets\": []
    }
  }"
  
  echo "📤 Executing withdraw_liquidity transaction..."
  
  $BINARY tx wasm execute "$pair_addr" \
    "$msg_json" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    --amount "$lp_amount$lp_denom" \
    -y -o json | jq
}

# Execute swap transaction
# Usage: swap <offer_amount> <offer_denom> [max_spread] [pair_contract_addr]
swap() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 swap <offer_amount> <offer_denom> [max_spread] [pair_contract_addr]"; echo "Example: $0 swap 100 uzig 0.01"; exit 1; }
  
  local offer_amount="$1"
  local offer_denom="$2"
  local max_spread="${3:-}"
  local pair_addr="${4:-$DEFAULT_PAIR_ADDR}"
  
  echo "🔄 Executing swap: $pair_addr"
  echo "Offer: $offer_amount $offer_denom"
  [ -n "$max_spread" ] && echo "Max spread: $max_spread"
  
  # Build the offer asset
  local offer_asset="{\"info\": {\"native_token\": {\"denom\": \"$offer_denom\"}}, \"amount\": \"$offer_amount\"}"
  
  # Build the message
  local msg_json="{
    \"swap\": {
      \"offer_asset\": $offer_asset"
  
  # Add max spread if specified
  if [ -n "$max_spread" ]; then
    msg_json="$msg_json,
      \"max_spread\": \"$max_spread\""
  fi
  
  msg_json="$msg_json
    }
  }"
  
  echo "📤 Executing swap transaction..."
  
  $BINARY tx wasm execute "$pair_addr" \
    "$msg_json" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    --amount "$offer_amount$offer_denom" \
    -y -o json | jq
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo ""
  echo "Default pair address: $DEFAULT_PAIR_ADDR"
  echo ""
  echo "Factory Queries:"
  echo "  query_all_pairs                                    - Query all pairs from factory"
  echo "  query_stable_pairs                                 - Query stable pairs from factory"
  echo "  query_factory_config                               - Query factory configuration"
  echo "  query_factory_fee_info                             - Query factory fee info for stable pairs"
  echo "  query_pair_by_assets <asset1> <asset2>            - Query pair by asset infos"
  echo ""
  echo "Pair Queries (uses default pair address if not specified):"
  echo "  query_pair [pair_addr]                             - Query pair info"
  echo "  query_pool [pair_addr]                             - Query pool info"
  echo "  query_config [pair_addr]                           - Query pair configuration"
  echo "  query_cumulative_prices [pair_addr]                - Query cumulative prices"
  echo "  query_compute_d [pair_addr]                        - Query compute D (stable pool)"
  echo ""
  echo "Swap Queries:"
  echo "  query_simulation <offer_amount> <offer_denom> [max_spread] [pair_addr]     - Query swap simulation"
  echo "  query_reverse_simulation <ask_asset> [offer_asset] [pair_addr] - Query reverse swap simulation"
  echo ""
  echo "Liquidity Queries:"
  echo "  query_share <lp_amount> [pair_addr]                - Query share calculation"
  echo "  query_simulate_provide <assets> [pair_addr]        - Query simulate provide liquidity"
  echo "  query_simulate_withdraw <lp_amount> [pair_addr]    - Query simulate withdraw liquidity"
  echo ""
  echo "Oracle Queries:"
  echo "  query_observe <seconds_ago> [pair_addr]            - Query observations (oracle data)"
  echo ""
  echo "Liquidity Transactions:"
  echo "  provide_liquidity <amount1> <denom1> <amount2> <denom2> [pair_addr] [receiver] - Provide liquidity"
  echo "  remove_liquidity <lp_amount> [pair_addr]           - Remove liquidity"
  echo ""
  echo "Swap Transactions:"
  echo "  swap <offer_amount> <offer_denom> [max_spread] [pair_addr] - Execute swap"
  echo ""
  echo "Examples:"
  echo "  $0 query_all_pairs"
  echo "  $0 query_stable_pairs"
  echo "  $0 query_pair"
  echo "  $0 query_pool"
  echo "  $0 query_simulation 100 uzig 0.01"
  echo "  $0 provide_liquidity 10000 coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.stable101 10000 uzig"
  echo "  $0 remove_liquidity 1000000"
  echo "  $0 swap 100 uzig 0.01"
  echo "  $0 query_pair_by_assets '{\"native_token\": {\"denom\": \"uzig\"}}' '{\"native_token\": {\"denom\": \"coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.stable101\"}}'"
  exit 0
fi

case "$1" in
  # Factory queries
  query_all_pairs)           query_all_pairs ;;
  query_stable_pairs)        query_stable_pairs ;;
  query_factory_config)      query_factory_config ;;
  query_factory_fee_info)    query_factory_fee_info ;;
  query_pair_by_assets)      shift; query_pair_by_assets "$@" ;;
  
  # Pair queries
  query_pair)                shift; query_pair "$@" ;;
  query_pool)                shift; query_pool "$@" ;;
  query_config)              shift; query_config "$@" ;;
  query_cumulative_prices)   shift; query_cumulative_prices "$@" ;;
  query_compute_d)           shift; query_compute_d "$@" ;;
  
  # Swap queries
  query_simulation)          shift; query_simulation "$@" ;;
  query_reverse_simulation)  shift; query_reverse_simulation "$@" ;;
  
  # Liquidity queries
  query_share)               shift; query_share "$@" ;;
  query_simulate_provide)    shift; query_simulate_provide "$@" ;;
  query_simulate_withdraw)   shift; query_simulate_withdraw "$@" ;;
  
  # Oracle queries
  query_observe)             shift; query_observe "$@" ;;
  
  # Liquidity transactions
  provide_liquidity)         shift; provide_liquidity "$@" ;;
  remove_liquidity)          shift; remove_liquidity "$@" ;;
  swap)                      shift; swap "$@" ;;
  
  *) echo "Unknown command: $1"; exit 1 ;;
esac 