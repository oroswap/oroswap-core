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

# Transaction settings
TX_FEES="$FEES"
GAS_ADJUSTMENT="$GAS_ADJUSTMENT"


# Default pair config parameters (for adding/updating concentrated)
PAIR_CODE_ID=114                  # replace with your concentrated PCL code ID (e.g. 13)
CONC_PAIR_CODE_ID=115          # replace with your concentrated PCL code ID (e.g. 13)
STABLE_PAIR_CODE_ID=139        # replace with your stable pair code ID

TOTAL_FEE_BPS=100               # e.g. 1.00%
MAKER_FEE_BPS=1000              # e.g. 10% of total fee
PERMISSIONED=false
IS_DISABLED=false
IS_GEN_DISABLED=false

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
# Usage: fetch_pairs [start_after] [limit]
fetch_pairs() {
  local start_after=${1:-null}
  local limit=${2:-100}
  echo "🔍 Fetching all pairs (start_after=$start_after, limit=$limit)..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs":{"start_after":'"$start_after"' ,"limit":'"$limit"'}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Fetch XYK pairs only
# Usage: fetch_pairs_by_type [start_after] [limit]
fetch_pairs_by_type() {
  local start_after=${1:-null}
  local limit=${2:-100}
  echo "🔍 Fetching XYK pairs (start_after=$start_after, limit=$limit)..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{"pairs_by_type":{"pair_type":{"xyk_30":{}},"start_after":'"$start_after"' ,"limit":'"$limit"'}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# ─── PAUSE FUNCTIONS ────────────────────────────────────────────────────────

# Query pause authorities
query_pause_authorities() {
  echo "🔍 Querying pause authorities..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{
      "pause_authorities": {}
    }' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query if pair is paused
# Usage: query_is_pair_paused <asset1_denom> <asset2_denom> <pair_type>
query_is_pair_paused() {
  [ "$#" -eq 3 ] || { echo "Usage: $0 query_is_pair_paused <asset1_denom> <asset2_denom> <pair_type>"; echo "Example: $0 query_is_pair_paused uzig coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101 '{\"custom\": \"xyk_30\"}'"; exit 1; }
  local asset1_denom="$1"
  local asset2_denom="$2"
  local pair_type="$3"
  
  echo "🔍 Querying if pair is paused: $asset1_denom ↔ $asset2_denom ($pair_type)"
  $BINARY query wasm contract-state smart "$FACTORY" \
    "{
      \"is_pair_paused\": {
        \"asset_infos\": [
          {\"native_token\": {\"denom\": \"$asset1_denom\"}},
          {\"native_token\": {\"denom\": \"$asset2_denom\"}}
        ],
        \"pair_type\": $pair_type
      }
    }" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query paused pairs count
query_paused_pairs_count() {
  echo "🔍 Querying paused pairs count..."
  $BINARY query wasm contract-state smart "$FACTORY" \
    '{
      "paused_pairs_count": {}
    }' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Add pause authorities
# Usage: add_pause_authorities <authority1> [authority2] [authority3] ...
add_pause_authorities() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 add_pause_authorities <authority1> [authority2] [authority3] ..."; echo "Example: $0 add_pause_authorities zig1abc123 zig1def456"; exit 1; }
  
  local authorities=()
  for auth in "$@"; do
    authorities+=("\"$auth\"")
  done
  
  local authorities_json=$(IFS=,; echo "[${authorities[*]}]")
  
  echo "▶️ Adding pause authorities: $*"
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"add_pause_authorities\": {
        \"authorities\": $authorities_json
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Remove pause authorities
# Usage: remove_pause_authorities <authority1> [authority2] [authority3] ...
remove_pause_authorities() {
  [ "$#" -ge 1 ] || { echo "Usage: $0 remove_pause_authorities <authority1> [authority2] [authority3] ..."; echo "Example: $0 remove_pause_authorities zig1abc123 zig1def456"; exit 1; }
  
  local authorities=()
  for auth in "$@"; do
    authorities+=("\"$auth\"")
  done
  
  local authorities_json=$(IFS=,; echo "[${authorities[*]}]")
  
  echo "▶️ Removing pause authorities: $*"
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"remove_pause_authorities\": {
        \"authorities\": $authorities_json
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Pause specific pair
# Usage: pause_pair <asset1_denom> <asset2_denom> <pair_type>
pause_pair() {
  [ "$#" -eq 3 ] || { echo "Usage: $0 pause_pair <asset1_denom> <asset2_denom> <pair_type>"; echo "Example: $0 pause_pair uzig coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101 '{\"custom\": \"xyk_30\"}'"; exit 1; }
  local asset1_denom="$1"
  local asset2_denom="$2"
  local pair_type="$3"
  
  echo "⏸️ Pausing pair: $asset1_denom ↔ $asset2_denom ($pair_type)"
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"pause_pair\": {
        \"asset_infos\": [
          {\"native_token\": {\"denom\": \"$asset1_denom\"}},
          {\"native_token\": {\"denom\": \"$asset2_denom\"}}
        ],
        \"pair_type\": $pair_type
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Unpause specific pair
# Usage: unpause_pair <asset1_denom> <asset2_denom> <pair_type>
unpause_pair() {
  [ "$#" -eq 3 ] || { echo "Usage: $0 unpause_pair <asset1_denom> <asset2_denom> <pair_type>"; echo "Example: $0 unpause_pair uzig coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101 '{\"custom\": \"xyk_30\"}'"; exit 1; }
  local asset1_denom="$1"
  local asset2_denom="$2"
  local pair_type="$3"
  
  echo "▶️ Unpausing pair: $asset1_denom ↔ $asset2_denom ($pair_type)"
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"unpause_pair\": {
        \"asset_infos\": [
          {\"native_token\": {\"denom\": \"$asset1_denom\"}},
          {\"native_token\": {\"denom\": \"$asset2_denom\"}}
        ],
        \"pair_type\": $pair_type
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Pause pairs batch
# Usage: pause_pairs_batch [batch_size]
pause_pairs_batch() {
  local batch_size="${1:-50}"
  
  echo "⏸️ Pausing pairs in batch (batch_size: $batch_size)"
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"pause_pairs_batch\": {
        \"batch_size\": $batch_size
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Unpause pairs batch
# Usage: unpause_pairs_batch [batch_size]
unpause_pairs_batch() {
  local batch_size="${1:-50}"
  
  echo "▶️ Unpausing pairs in batch (batch_size: $batch_size)"
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"unpause_pairs_batch\": {
        \"batch_size\": $batch_size
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Update pool_creation_fee for the XYK config
# Usage: update_pool_creation_fee <new_fee_amount>
update_pool_creation_fee() {
  [ $# -eq 1 ] || { echo "Usage: $0 update_pool_creation_fee <new_fee>"; exit 1; }
  local new_fee=$1
  echo "✏️ Updating pool_creation_fee to $new_fee..."
  $BINARY tx wasm execute "$FACTORY" \
    '{
      "update_pair_config": {
        "config": {
          "code_id": '"$PAIR_CODE_ID"',
          "pair_type": {"xyk":{}},
          "total_fee_bps": '"$TOTAL_FEE_BPS"',
          "maker_fee_bps": '"$MAKER_FEE_BPS"',
          "permissioned": '"$PERMISSIONED"',
          "is_disabled": '"$IS_DISABLED"',
          "is_generator_disabled": '"$IS_GEN_DISABLED"',
          "pool_creation_fee": "'"$new_fee"'"
        }
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Add a new concentrated pair config (hardcoded type "concentrated")
# Usage: add_concentrated_pair_config
add_concentrated_pair_config() {
  echo "✏️ Adding concentrated pair config with code_id=$PAIR_CODE_ID, total_fee_bps=$TOTAL_FEE_BPS, maker_fee_bps=$MAKER_FEE_BPS..."
  $BINARY tx wasm execute "$FACTORY" \
    '{
      "update_pair_config": {
        "config": {
          "code_id": '"$CONC_PAIR_CODE_ID"',
          "pair_type": { "custom": "concentrated" },
          "total_fee_bps": '"$TOTAL_FEE_BPS"',
          "maker_fee_bps": '"$MAKER_FEE_BPS"',
          "permissioned": '"$PERMISSIONED"',
          "is_disabled": '"$IS_DISABLED"',
          "is_generator_disabled": '"$IS_GEN_DISABLED"'
        }
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Add a new stable pair config
# Usage: add_stable_pair_config
add_stable_pair_config() {
  echo "✏️ Adding stable pair config with code_id=$STABLE_PAIR_CODE_ID, total_fee_bps=$TOTAL_FEE_BPS, maker_fee_bps=$MAKER_FEE_BPS..."
  $BINARY tx wasm execute "$FACTORY" \
    '{
      "update_pair_config": {
        "config": {
          "code_id": '"$STABLE_PAIR_CODE_ID"',
          "pair_type": { "stable": {} },
          "total_fee_bps": '"$TOTAL_FEE_BPS"',
          "maker_fee_bps": '"$MAKER_FEE_BPS"',
          "permissioned": '"$PERMISSIONED"',
          "is_disabled": '"$IS_DISABLED"',
          "is_generator_disabled": '"$IS_GEN_DISABLED"',
          "pool_creation_fee": "1000000"
        }
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Add a new XYK pair config with custom fee tier
add_custom_xyk_pair_config() {
  echo "✏️ Adding custom XYK pair config with code_id=$PAIR_CODE_ID, total_fee_bps=$TOTAL_FEE_BPS, maker_fee_bps=$MAKER_FEE_BPS..."
  $BINARY tx wasm execute "$FACTORY" \
    '{
      "update_pair_config": {
        "config": {
          "code_id": '"$PAIR_CODE_ID"',
          "pair_type": { "custom": "xyk_30" },
          "total_fee_bps": '"$TOTAL_FEE_BPS"',
          "maker_fee_bps": '"$MAKER_FEE_BPS"',
          "permissioned": '"$PERMISSIONED"',
          "is_disabled": '"$IS_DISABLED"',
          "is_generator_disabled": '"$IS_GEN_DISABLED"',
          "pool_creation_fee": "1000000"
        }
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Update all pair configs to use new code IDs
# Usage: update_all_pair_configs
update_all_pair_configs() {
  echo "✏️ Updating all pair configs to use new code IDs..."
  echo "  - XYK pairs: code_id → $PAIR_CODE_ID"
  echo "  - XYK_30 pairs: code_id → $PAIR_CODE_ID" 
  echo "  - Concentrated pairs: code_id → $CONC_PAIR_CODE_ID"
  echo "  - Stable pairs: code_id → $STABLE_PAIR_CODE_ID"
  
  # # Update XYK pair config
  # echo "🔄 Updating XYK pair config..."
  # $BINARY tx wasm execute "$FACTORY" \
  #   "{
  #     \"update_pair_config\": {
  #       \"config\": {
  #         \"code_id\": $PAIR_CODE_ID,
  #         \"pair_type\": {\"xyk\":{}},
  #         \"total_fee_bps\": 10,
  #         \"maker_fee_bps\": 2000,
  #         \"is_disabled\": false,
  #         \"is_generator_disabled\": false,
  #         \"permissioned\": false,
  #         \"pool_creation_fee\": \"1000000\"
  #       }
  #     }
  #   }" \
  #   --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  #   --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  #   --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
  #   -y -o json | jq
    
  # echo "⏳ Waiting for transaction to be processed..."
  # sleep 3
    
  # # Update XYK_30 pair config
  # echo "🔄 Updating XYK_30 pair config..."
  # $BINARY tx wasm execute "$FACTORY" \
  #   "{
  #     \"update_pair_config\": {
  #       \"config\": {
  #         \"code_id\": $PAIR_CODE_ID,
  #         \"pair_type\": {\"custom\": \"xyk_30\"},
  #         \"total_fee_bps\": 30,
  #         \"maker_fee_bps\": 2000,
  #         \"is_disabled\": false,
  #         \"is_generator_disabled\": false,
  #         \"permissioned\": false,
  #         \"pool_creation_fee\": \"1000000\"
  #       }
  #     }
  #   }" \
  #   --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  #   --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  #   --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
  #   -y -o json | jq
    
  # echo "⏳ Waiting for transaction to be processed..."
  # sleep 3
    
  # Update Concentrated pair config
  echo "🔄 Updating Concentrated pair config..."
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"update_pair_config\": {
        \"config\": {
          \"code_id\": $CONC_PAIR_CODE_ID,
          \"pair_type\": {\"custom\": \"concentrated\"},
          \"total_fee_bps\": 100,
          \"maker_fee_bps\": 1000,
          \"is_disabled\": false,
          \"is_generator_disabled\": false,
          \"permissioned\": false,
          \"pool_creation_fee\": \"1000000\"
        }
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
    
  echo "✅ All pair configs updated!"
  echo "🔍 Verifying updated config..."
  fetch_config
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

# Pause pair by contract address
# Usage: pause_pair_by_address <pair_contract_address>
pause_pair_by_address() {
  [ "$#" -eq 1 ] || { echo "Usage: $0 pause_pair_by_address <pair_contract_address>"; echo "Example: $0 pause_pair_by_address zig1tqwwyth34550lg2437m05mjnjp8w7h5ka7m70jtzpxn4uh2ktsmqg4j0v6"; exit 1; }
  local pair_address="$1"
  
  echo "🔍 Querying pair info for address: $pair_address"
  
  # Query pair info directly from pair contract
  local pair_info_json
  pair_info_json=$($BINARY query wasm contract-state smart "$pair_address" \
    '{"pair": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json)
  
  if [[ -z "$pair_info_json" || "$(echo "$pair_info_json" | jq -r '.data')" == "null" ]]; then
    echo "❌ Error: Could not query pair info from address $pair_address"
    exit 1
  fi
  
  # Extract asset infos and pair type
  local asset1_denom asset2_denom pair_type
  asset1_denom=$(echo "$pair_info_json" | jq -r '.data.asset_infos[0].native_token.denom')
  asset2_denom=$(echo "$pair_info_json" | jq -r '.data.asset_infos[1].native_token.denom')
  pair_type=$(echo "$pair_info_json" | jq -c '.data.pair_type')
  
  echo "📋 Found pair: $asset1_denom ↔ $asset2_denom ($pair_type)"
  echo "⏸️ Pausing pair by address: $pair_address"
  
  # Call the existing pause_pair function
  pause_pair "$asset1_denom" "$asset2_denom" "$pair_type"
}

# Unpause pair by contract address
# Usage: unpause_pair_by_address <pair_contract_address>
unpause_pair_by_address() {
  [ "$#" -eq 1 ] || { echo "Usage: $0 unpause_pair_by_address <pair_contract_address>"; echo "Example: $0 unpause_pair_by_address zig1tqwwyth34550lg2437m05mjnjp8w7h5ka7m70jtzpxn4uh2ktsmqg4j0v6"; exit 1; }
  local pair_address="$1"
  
  echo "🔍 Querying pair info for address: $pair_address"
  
  # Query pair info directly from pair contract
  local pair_info_json
  pair_info_json=$($BINARY query wasm contract-state smart "$pair_address" \
    '{"pair": {}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json)
  
  if [[ -z "$pair_info_json" || "$(echo "$pair_info_json" | jq -r '.data')" == "null" ]]; then
    echo "❌ Error: Could not query pair info from address $pair_address"
    exit 1
  fi
  
  # Extract asset infos and pair type
  local asset1_denom asset2_denom pair_type
  asset1_denom=$(echo "$pair_info_json" | jq -r '.data.asset_infos[0].native_token.denom')
  asset2_denom=$(echo "$pair_info_json" | jq -r '.data.asset_infos[1].native_token.denom')
  pair_type=$(echo "$pair_info_json" | jq -c '.data.pair_type')
  
  echo "📋 Found pair: $asset1_denom ↔ $asset2_denom ($pair_type)"
  echo "▶️ Unpausing pair by address: $pair_address"
  
  # Call the existing unpause_pair function
  unpause_pair "$asset1_denom" "$asset2_denom" "$pair_type"
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  list_codes"                                  # List all uploaded code IDs
  echo "  fetch_config"                                # Fetch factory configuration
  echo "  fetch_pairs [start_after] [limit]"          # List all pairs
  echo "  fetch_pairs_by_type [start_after] [limit]"  # List XYK pairs
  echo "  update_pool_creation_fee <new_fee>"         # Update pool creation fee
  echo "  add_concentrated_pair_config"               # Add a concentrated pair config
  echo "  add_stable_pair_config"                     # Add a stable pair config
  echo "  add_custom_xyk_pair_config"               # Add a custom XYK pair config
  echo "  update_all_pair_configs"                    # Update all pair configs to new code IDs
  echo "  update_config <fee_addr> <gen_addr|null> <reg_addr|null>"  # Update fee/generator/registry
  echo ""
  echo "🔒 PAUSE FUNCTIONS:"
  echo "  query_pause_authorities                    - Query all pause authorities"
  echo "  query_is_pair_paused <asset1> <asset2> <pair_type> - Check if pair is paused"
  echo "  query_paused_pairs_count                   - Get count of paused pairs"
  echo "  add_pause_authorities <auth1> [auth2] ...  - Add pause authorities (owner only)"
  echo "  remove_pause_authorities <auth1> [auth2] ... - Remove pause authorities (owner only)"
  echo "  pause_pair <asset1> <asset2> <pair_type>   - Pause specific pair (pause authorities)"
  echo "  unpause_pair <asset1> <asset2> <pair_type> - Unpause specific pair (factory admin only)"
  echo "  pause_pairs_batch [batch_size]             - Pause pairs in batch (pause authorities)"
  echo "  unpause_pairs_batch [batch_size]           - Unpause pairs in batch (factory admin only)"
  echo "  pause_pair_by_address <pair_address>       - Pause pair by address (pause authorities)"
  echo "  unpause_pair_by_address <pair_address>     - Unpause pair by address (factory admin only)"
  echo ""
  echo "Examples:"
  echo "  $0 query_is_pair_paused uzig coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101 '{\"custom\": \"xyk_30\"}'"
  echo "  $0 pause_pair uzig coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101 '{\"custom\": \"xyk_30\"}'  # Requires pause authority"
  echo "  $0 unpause_pair uzig coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101 '{\"custom\": \"xyk_30\"}'  # Requires factory admin"
  echo "  $0 pause_pair_by_address zig1gw8v4n0pal67ds3uncn8lxur544e3gd9ue3kzpnwx9smn2la6gqq6vqpml  # Requires pause authority"
  echo "  $0 add_pause_authorities zig1abc123 zig1def456  # Requires factory admin"
  exit 0
fi

case "$1" in
  list_codes)                         list_codes ;;
  fetch_config)                       fetch_config ;;
  fetch_pairs)                        shift; fetch_pairs "$@" ;;
  fetch_pairs_by_type)                shift; fetch_pairs_by_type "$@" ;;
  update_pool_creation_fee)           shift; update_pool_creation_fee "$@" ;;
  add_concentrated_pair_config)       add_concentrated_pair_config ;;
  add_stable_pair_config)             add_stable_pair_config ;;
  add_custom_xyk_pair_config)       add_custom_xyk_pair_config ;;
  update_all_pair_configs)             update_all_pair_configs ;;
  update_config)                      shift; update_config "$@" ;;
  query_pause_authorities)            query_pause_authorities ;;
  query_is_pair_paused)               shift; query_is_pair_paused "$@" ;;
  query_paused_pairs_count)           query_paused_pairs_count ;;
  add_pause_authorities)              shift; add_pause_authorities "$@" ;;
  remove_pause_authorities)           shift; remove_pause_authorities "$@" ;;
  pause_pair)                         shift; pause_pair "$@" ;;
  unpause_pair)                       shift; unpause_pair "$@" ;;
  pause_pairs_batch)                  shift; pause_pairs_batch "$@" ;;
  unpause_pairs_batch)                 shift; unpause_pairs_batch "$@" ;;
  pause_pair_by_address)              shift; pause_pair_by_address "$@" ;;
  unpause_pair_by_address)             shift; unpause_pair_by_address "$@" ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
