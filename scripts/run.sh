#!/usr/bin/env bash
set -euo pipefail

# Add Go binary path to PATH
export PATH="$HOME/go/bin:$PATH"

# Load environment variables from .env
if [ -f ../scripts/.env ]; then
  export $(grep -v '^#' ../scripts/.env | xargs)
fi

# ──────────────────────────────────────────────────────────────────────────────
# CONFIGURATION
# ──────────────────────────────────────────────────────────────────────────────

# Chain configuration
export CHAIN_ID=${CHAIN_ID:-"localosmosis"}
export CHAIN_CMD=${CHAIN_CMD:-"osmosisd"}
export RPC_URL=${RPC_URL:-"http://localhost:26657"}
export DENOM=${DENOM:-"uosmo"}
export GAS_PRICES=${GAS_PRICES:-"50${DENOM}"}
export KEY_NAME=${KEY_NAME:-"test"}
export ADMIN_ADDRESS=${ADMIN_ADDRESS:-$($CHAIN_CMD keys show $KEY_NAME -a)}
export BROADCAST_MODE=${BROADCAST_MODE:-"sync"}

# Timeout configuration (in seconds)
export TX_TIMEOUT=${TX_TIMEOUT:-30}
export POLL_INTERVAL=${POLL_INTERVAL:-2}

# Contract code IDs
export FACTORY_CODE_ID=${FACTORY_CODE_ID:-2}
export CW20_CODE_ID=${CW20_CODE_ID:-3}
export PAIR_CODE_ID=${PAIR_CODE_ID:-4}

# Paths to compiled wasm files
export WASM_DIR=${WASM_DIR:-"../artifacts"}
export FACTORY_WASM=${FACTORY_WASM:-"${WASM_DIR}/astroport_factory-aarch64.wasm"}
export PAIR_WASM=${PAIR_WASM:-"${WASM_DIR}/astroport_pair_zigchain-aarch64.wasm"}
export CW20_WASM=${CW20_WASM:-"${WASM_DIR}/cw20_base.wasm"}

# Define factory_init_msg globally
factory_init_msg='{
    "token_code_id": '"$CW20_CODE_ID"',
    "whitelist_code_id": 0,
    "coin_registry_address": "'"$ADMIN_ADDRESS"'",
    "fee_address": "'"$ADMIN_ADDRESS"'",
    "owner": "'"$ADMIN_ADDRESS"'",
    "generator_address": "'"$ADMIN_ADDRESS"'",
    "pair_configs": [{
        "code_id": '"$PAIR_CODE_ID"',
        "pair_type": {"xyk": {}},
        "total_fee_bps": 100,
        "maker_fee_bps": 10,
        "is_disabled": false
    }]
}'

# JSON file to store deployment state
STATE_FILE="deployment_state.json"

# Initialize the state file if it doesn't exist
initialize_state_file() {
    if [ ! -f "$STATE_FILE" ]; then
        echo "{}" > "$STATE_FILE"
    fi
}

# Helper function to update the JSON state file
update_state() {
    local key=$1
    local value=$2
    jq --arg key "$key" --arg value "$value" '.[$key] = $value' "$STATE_FILE" > "${STATE_FILE}.tmp" && mv "${STATE_FILE}.tmp" "$STATE_FILE"
}

# Helper function to read a value from the JSON state file
read_state() {
    local key=$1
    jq -r --arg key "$key" '.[$key] // empty' "$STATE_FILE"
}

# ──────────────────────────────────────────────────────────────────────────────
# FUNCTIONS
# ──────────────────────────────────────────────────────────────────────────────

# Helper function to wait for transaction
wait_for_tx() {
    local tx_hash=$1
    local timeout=$2
    local start_time=$(date +%s)
    local end_time=$((start_time + timeout))

    # echo "⏳ Waiting for transaction $tx_hash to be processed..."

    while true; do
        local now=$(date +%s)
        if (( now >= end_time )); then
            echo "❌ Timeout waiting for transaction $tx_hash"
            return 1
        fi

        # Filter out gas estimate lines before parsing JSON
        local tx_result=$($CHAIN_CMD q tx "$tx_hash" --node "$RPC_URL" --output json 2>/dev/null | grep -v '^gas estimate:' || true)

        if [[ -n "$tx_result" && "$tx_result" != "null" ]]; then
            # Check if valid JSON
            if jq -e . >/dev/null 2>&1 <<<"$tx_result"; then
                # Check if transaction failed
                local code=$(echo "$tx_result" | jq -r '.code')
                if [[ "$code" != "0" && "$code" != "null" ]]; then
                    local raw_log=$(echo "$tx_result" | jq -r '.raw_log')
                    echo "❌ Transaction failed (code $code): $raw_log"
                    return 1
                fi

                echo "$tx_result"
                return 0
            fi
        fi

        sleep "$POLL_INTERVAL"
    done
}

# Helper function to extract code_id from tx result
get_code_id() {
    local tx_result=$1
    echo "$tx_result" | \
        jq -r '.events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value' || echo ""
}

# Helper function to extract contract_address from tx result
get_contract_address() {
    local tx_result=$1
    #  store trx result
    echo "$tx_result" | \
        jq -r '.events[] | select(.type == "instantiate") | .attributes[] | select(.key == "_contract_address") | .value'
}

# Store contract wasm file and return code_id
store_contract() {
    local wasm_file=$1
    local fees=$2
    local label=$3

    # Convert label to snake_case for JSON key
    local key=$(echo "$label" | tr '[:upper:]' '[:lower:]' | tr ' ' '_')_code_id

    # Check if the contract is already stored
    local existing_code_id=$(read_state "$key")
    if [ -n "$existing_code_id" ]; then
        echo "✅ ${label} already stored with code_id: ${existing_code_id}" >&2
        # Return only the code_id
        echo "$existing_code_id"
        return 0
    fi

    echo "⏳ Storing ${label} contract..." >&2

    # Submit the transaction
    local tx_result=$($CHAIN_CMD tx wasm store "$wasm_file" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --gas auto --gas-adjustment 1.3 \
        --fees "${fees}${DENOM}" \
        --broadcast-mode "$BROADCAST_MODE" \
        -y -o json 2>&1 | grep -v '^gas estimate:' || true)

    echo "$tx_result" > tx_result.json

    # Extract txhash
    local tx_hash=$(echo "$tx_result" | jq -r '.txhash')
    if [[ -z "$tx_hash" ]]; then
        echo "❌ Failed to extract tx_hash from transaction result" >&2
        return 1
    fi
    echo "Transaction hash: $tx_hash"

    # Wait for transaction to be processed
    local tx_data=$(wait_for_tx "$tx_hash" "$TX_TIMEOUT") || return 1
    echo "Transaction data: $tx_data"

    # Extract code_id
    local code_id=$(get_code_id "$tx_data")
    if [[ -z "$code_id" ]]; then
        echo "❌ Failed to extract code_id from transaction" >&2
        return 1
    fi

    # Save code_id to the state file
    update_state "$key" "$code_id"

    echo "✅ ${label} stored with code_id: ${code_id}" >&2
    # Return only the code_id
    echo "$code_id"
}

# Instantiate a contract
instantiate_contract() {
    local code_id=$1
    local init_msg=$2
    local label=$3
    local admin=$4

    # Convert label to snake_case for JSON key
    local key=$(echo "$label" | tr '[:upper:]' '[:lower:]' | tr ' ' '_')_address

    # Check if the contract is already instantiated
    local existing_address=$(read_state "$key")
    if [ -n "$existing_address" ]; then
        echo "✅ ${label} already instantiated at: ${existing_address}" >&2
        echo "$existing_address"
        return 0
    fi

    echo "⏳ Instantiating ${label} (code_id=${code_id}) ..." >&2
    local tx_result=$($CHAIN_CMD tx wasm instantiate "$code_id" "$init_msg" \
        --label "$label" \
        --admin "$admin" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode "$BROADCAST_MODE" \
        --gas auto --gas-adjustment 1.3 --gas-prices "$GAS_PRICES" \
        -y -o json)

    # Extract txhash
    local tx_hash=$(echo "$tx_result" | jq -r '.txhash')
    if [[ -z "$tx_hash" ]]; then
        echo "❌ Failed to extract tx_hash from transaction result"
        return 1
    fi
    echo "Transaction hash: $tx_hash"

    # Wait for transaction to be processed
    local tx_data=$(wait_for_tx "$tx_hash" "$TX_TIMEOUT") || return 1

    # Extract contract address
    local contract_address=$(get_contract_address "$tx_data")
    if [[ -z "$contract_address" ]]; then
        echo "❌ Failed to extract contract address from transaction"
        return 1
    fi

    # Save contract address to the state file
    update_state "$key" "$contract_address"

    echo "✅ ${label} instantiated at: ${contract_address}"
    echo "$contract_address"
}

# Execute a contract method
execute_contract() {
    local contract_address=$1
    local exec_msg=$2
    local action_label=$3

    echo "⏳ Executing ${action_label} on ${contract_address}"
    $CHAIN_CMD tx wasm execute "$contract_address" "$exec_msg" \
        --amount 1000factory/zig1pay2fdaqxzw9sts4q8cq4ycq7wtm0uc3canu2vsemmfgggldyrpqfwa6yq/oroshare \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$RPC_URL" \
        --broadcast-mode "$BROADCAST_MODE" \
        --gas 1000000 --gas-adjustment 1.3 --gas-prices "$GAS_PRICES" \
        -y -o json | jq .

    echo "✅ ${action_label} executed successfully"
}

# ──────────────────────────────────────────────────────────────────────────────
# MAIN DEPLOYMENT FLOW
# ──────────────────────────────────────────────────────────────────────────────

deploy_all() {
    echo "🚀 Starting full deployment on ${CHAIN_ID}"

    # Initialize the state file
    initialize_state_file

    # 1. Store contracts
    local factory_code_id=$(store_contract "$FACTORY_WASM" 8000 "Astroport Factory")
    local pair_code_id=$(store_contract "$PAIR_WASM" 10000 "Astroport Pair")
    local cw20_code_id=$(store_contract "$CW20_WASM" 10000 "CW20 Base")

    factory_init_msg=$(echo "$factory_init_msg" | \
        jq --arg pair_code_id "$pair_code_id" --arg cw20_code_id "$cw20_code_id" \
        '.token_code_id = ($cw20_code_id | tonumber) | .pair_configs[0].code_id = ($pair_code_id | tonumber)')

    echo "Factory init msg: $factory_init_msg"

    # 2. Instantiate Factory
    local factory_address=$(instantiate_contract "$factory_code_id" "$factory_init_msg" "astro-factory" "$ADMIN_ADDRESS")
    echo "Factory address: $factory_address" >&2

    # 3. Create a pair through factory
    local create_pair_msg='{
        "create_pair": {
            "pair_type": {"xyk": {}},
            # "fee: {
            #     "total_fee_bps": 100,
            #     "maker_fee_bps": 10
            # },
            "asset_infos": [
                {"native_token": {"denom": "'"$DENOM"'"}},
                {"native_token": {"denom": "factory/zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl/dai"}}
            ]
        }
    }'
    echo "Create pair msg: $create_pair_msg"

    # execute_contract "$factory_address" "$create_pair_msg" "Create Pair"


    local add_liquidity_msg='{
    "provide_liquidity": {
        "assets": [
            {
            "info": {
                "native_token": {
                    "denom": "'"$DENOM"'"
                }
            },
            "amount": "1000000"
            },
            {
            "info": {
                "native_token": {
                    "denom": "factory/zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl/dai"
                }
            },
            "amount": "1000000"
            }
        ]
    }
    }'
    echo "Add liquidity msg: $add_liquidity_msg"

    # execute_contract "zig1pay2fdaqxzw9sts4q8cq4ycq7wtm0uc3canu2vsemmfgggldyrpqfwa6yq" "$add_liquidity_msg" "Add Liquidity"

    local remove_liquidity_msg='{
    "withdraw_liquidity": {}
    }'
    echo "Remove liquidity msg: $remove_liquidity_msg"

    execute_contract "zig1pay2fdaqxzw9sts4q8cq4ycq7wtm0uc3canu2vsemmfgggldyrpqfwa6yq" "$remove_liquidity_msg" "Remove Liquidity"

    echo "🎉 Deployment completed successfully!"
}

# ──────────────────────────────────────────────────────────────────────────────
# MODULAR EXECUTION
# ──────────────────────────────────────────────────────────────────────────────

# Parse command line arguments
case "${1:-}" in
    store-factory)
        store_contract "$FACTORY_WASM" 8000 "Astroport Factory"
        ;;
    store-pair)
        store_contract "$PAIR_WASM" 10000 "Astroport Pair"
        ;;
    store-cw20)
        store_contract "$CW20_WASM" 10000 "CW20 Base"
        ;;
    instantiate-factory)
        if [ -z "$2" ]; then
            echo "Usage: $0 instantiate-factory <code_id>"
            exit 1
        fi
        # Similar init msg as in deploy_all
        instantiate_contract "$2" "$factory_init_msg" "astro-factory" "$ADMIN_ADDRESS"
        ;;
    create-pair)
        if [ -z "$2" ]; then
            echo "Usage: $0 create-pair <factory_address>"
            exit 1
        fi
        # Similar create pair msg as in deploy_all
        execute_contract "$2" "$create_pair_msg" "Create Pair"
        ;;
    all)
        deploy_all
        ;;
    *)
        echo "Usage: $0 {store-factory|store-pair|store-cw20|instantiate-factory|create-pair|all}"
        exit 1
        ;;
esac

# zigchaind tx bank send test zig15a5hep9exu588w7qczrgq2w57ljtg8kfq5kv2m2h94k9m8mxy6zsl3548p 1000000uzig \
#   --chain-id="zigchain" \
#   --node="https://rpc.oroswap.org" \
#   --gas="auto" \
#   --gas-adjustment="1.5" \
#   --gas-prices="1uzig" \
#   --broadcast-mode="sync" \
#   --from=test \
#   -y

# zigchaind query tx 1378643C1251D23CC8E5548216BB0EA62E1EE50EEA91E5B9C87D94F4B0F26EF6 --node="https://rpc.oroswap.org"

# # query balances
# zigchaind query bank balances zig15a5hep9exu588w7qczrgq2w57ljtg8kfq5kv2m2h94k9m8mxy6zsl3548p --node="https://rpc.oroswap.org"

zigchaind tx wasm instantiate 35 '{
  "name": "MyToken",
  "symbol": "MTK",
  "decimals": 6,
  "initial_balances": [
    {
      "address": "zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl",
      "amount": "1000000"
    }
  ],
  "mint": {
    "minter": "zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl"
  }
}' \
  --label "cw20-mytoken" \
  --admin "zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl" \
  --from "test" \
  --chain-id "zigchain" \
  --node "https://rpc.oroswap.org" \
  --gas "auto" \
  --gas-adjustment "1.3" \
  --gas-prices "0.025uzig" \
  -y -o json | jq .