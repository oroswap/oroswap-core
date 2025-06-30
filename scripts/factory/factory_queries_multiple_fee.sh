#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://devnet-rpc.zigchain.com"
CHAIN_ID="zig-devnet-1"
KEY_NAME="devnet-key"
KEYRING_BACKEND="test"
FACTORY="zig1wn625s4jcmvk0szpl85rj5azkfc6suyvf75q6vrddscjdphtve8sh9354q"

# Default fees
TX_FEES="20000uzig"
GAS_ADJUSTMENT="1.3"

# Default pair config parameters (for adding/updating concentrated)
PAIR_CODE_ID=8                  # replace with your concentrated PCL code ID (e.g. 13)
CONC_PAIR_CODE_ID=13            # replace with your concentrated PCL code ID (e.g. 13)

TOTAL_FEE_BPS=100               # e.g. 1.00%
MAKER_FEE_BPS=1000              # e.g. 10% of total fee
PERMISSIONED=false
IS_DISABLED=false
IS_GEN_DISABLED=false
POOL_CREATION_FEE="1000000"    # Default pool creation fee

# ─── Functions ──────────────────────────────────────────────────────────────

# List all uploaded code IDs
list_codes() {
  echo "🔍 Listing all wasm code IDs..."
  $BINARY query wasm list-code \
    --node "$RPC_URL" \
    --chain-id "$CHAIN_ID" \
    -o json | jq '.code_infos[] | .code_id'
}

# Fetch factory config
fetch_config() {
  echo "🔍 Fetching factory config..."
  $BINARY query wasm contract-state smart "$FACTORY" '{"config":{}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Fetch all pairs
# Usage: fetch_pairs [start_after_asset_infos] [start_after_pair_type] [limit]
fetch_pairs() {
  local start_after_asset_infos=${1:-null}
  local start_after_pair_type=${2:-null}
  local limit=${3:-100}
  
  # Build start_after JSON if provided
  local start_after_json="null"
  if [[ "$start_after_asset_infos" != "null" && "$start_after_pair_type" != "null" ]]; then
    start_after_json='{
      "asset_infos": '"$start_after_asset_infos"',
      "pair_type": '"$start_after_pair_type"'
    }'
  fi

  echo "🔍 Fetching all pairs (start_after=$start_after_json, limit=$limit)..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs":{"start_after":'"$start_after_json"',"limit":'"$limit"'}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Fetch pairs by assets
# Usage: fetch_pairs_by_assets <asset_infos_json>
fetch_pairs_by_assets() {
  local asset_infos=$1
  echo "🔍 Fetching pairs for assets $asset_infos..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs_by_assets":{"asset_infos":'"$asset_infos"'}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Fetch pairs by type
# Usage: fetch_pairs_by_type <pair_type> [start_after_asset_infos] [limit]
# Example: fetch_pairs_by_type '{"xyk":{}}' or fetch_pairs_by_type '{"custom":"xyk_30"}'
fetch_pairs_by_type() {
  local pair_type=$1
  local start_after_asset_infos=${2:-null}
  local limit=${3:-100}
  
  # Build start_after JSON if provided
  local start_after_json="null"
  if [[ "$start_after_asset_infos" != "null" ]]; then
    start_after_json='{
      "asset_infos": '"$start_after_asset_infos"',
      "pair_type": '"$pair_type"'
    }'
  fi

  echo "🔍 Fetching pairs of type $pair_type (start_after=$start_after_json, limit=$limit)..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs":{"start_after":'"$start_after_json"',"limit":'"$limit"'}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq '.data.pairs[] | select(.pair_type == '"$pair_type"')' | jq -s '{data: {pairs: .}}'
}

# Update pair config
# Usage: update_pair_config <pair_type> <code_id> <total_fee_bps> <maker_fee_bps> <pool_creation_fee>
update_pair_config() {
  local pair_type=$1
  local code_id=$2
  local total_fee_bps=$3
  local maker_fee_bps=$4
  local pool_creation_fee=${5:-$POOL_CREATION_FEE}

  echo "✏️ Updating pair config for type $pair_type..."
  $BINARY tx wasm execute "$FACTORY" \
    '{
      "update_pair_config": {
        "config": {
          "code_id": '"$code_id"',
          "pair_type": '"$pair_type"',
          "total_fee_bps": '"$total_fee_bps"',
          "maker_fee_bps": '"$maker_fee_bps"',
          "permissioned": '"$PERMISSIONED"',
          "is_disabled": '"$IS_DISABLED"',
          "is_generator_disabled": '"$IS_GEN_DISABLED"',
          "pool_creation_fee": "'"$pool_creation_fee"'"
        }
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# ─── Update factory general configuration ─────────────────────────────────────
# Usage: update_config <fee_address> <generator_address|null> <coin_registry_address|null>
update_config() {
  if [[ $# -lt 1 ]]; then
    echo "Usage: $0 update_config <fee_address> <generator_address|null> <coin_registry_address|null>"
    exit 1
  fi
  local new_fee_addr=$1
  local gen_addr=${2:-null}
  local registry_addr=${3:-null}

  echo "✏️ Updating factory config:
    fee_address = $new_fee_addr
    generator_address = $gen_addr
    coin_registry_address = $registry_addr
  "

  # Build JSON fields for generator_address and coin_registry_address
  if [[ "$gen_addr" == "null" ]]; then
    gen_field='"generator_address": null'
  else
    gen_field='"generator_address": "'"$gen_addr"'"'
  fi

  if [[ "$registry_addr" == "null" ]]; then
    registry_field='"coin_registry_address": null'
  else
    registry_field='"coin_registry_address": "'"$registry_addr"'"'
  fi

  $BINARY tx wasm execute "$FACTORY" \
    '{
      "update_config": {
        "fee_address": "'"$new_fee_addr"'",
        '"$gen_field"',
        '"$registry_field"'
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  list_codes"                                  # List all uploaded code IDs
  echo "  fetch_config"                                # Fetch factory configuration
  echo "  fetch_pairs [start_after_asset_infos] [start_after_pair_type] [limit]"  # List all pairs
  echo "  fetch_pairs_by_assets <asset_infos_json>"    # List pairs by assets
  echo "  fetch_pairs_by_type <pair_type> [start_after_asset_infos] [limit]"  # List pairs by type
  echo "  update_pair_config <pair_type> <code_id> <total_fee_bps> <maker_fee_bps> [pool_creation_fee]"  # Update pair config
  echo "  update_config <fee_addr> <gen_addr|null> <reg_addr|null>"  # Update fee/generator/registry
  echo ""
  echo "Examples:"
  echo "  $0 fetch_pairs '[{\"native_token\":{\"denom\":\"uluna\"}},{\"token\":{\"contract_addr\":\"asset0001\"}}]' '{\"xyk\":{}}' 10"
  echo "  $0 fetch_pairs_by_assets '[{\"native_token\":{\"denom\":\"uluna\"}},{\"token\":{\"contract_addr\":\"asset0001\"}}]'"
  echo "  $0 fetch_pairs_by_type '{\"xyk\":{}}' '[{\"native_token\":{\"denom\":\"uluna\"}},{\"token\":{\"contract_addr\":\"asset0001\"}}]' 10"
  echo "  $0 update_pair_config '{\"xyk\":{}}' 8 100 1000 1000000"
  exit 0
fi

case "$1" in
  list_codes)                         list_codes ;;
  fetch_config)                       fetch_config ;;
  fetch_pairs)                        shift; fetch_pairs "$@" ;;
  fetch_pairs_by_assets)              shift; fetch_pairs_by_assets "$@" ;;
  fetch_pairs_by_type)                shift; fetch_pairs_by_type "$@" ;;
  update_pair_config)                 shift; update_pair_config "$@" ;;
  update_config)                      shift; update_config "$@" ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
