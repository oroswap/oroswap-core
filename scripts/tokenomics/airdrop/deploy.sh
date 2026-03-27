#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
# Source environment variables
source "$(dirname "$0")/../../devnet_testing.env"

# Path to the WASM file (assuming it's built or in artifacts)
WASM_PATH="$(dirname "$0")/../../../artifacts/oroswap_airdrop.wasm"

if [ ! -f "$WASM_PATH" ]; then
    echo "⚠️  WASM file not found at $WASM_PATH. Trying to find it in target/wasm32-unknown-unknown/release/..."
    WASM_PATH="$(dirname "$0")/../../../target/wasm32-unknown-unknown/release/oroswap_airdrop.wasm"
fi

if [ ! -f "$WASM_PATH" ]; then
    echo "❌ Error: WASM file not found. Please build the contract first."
    exit 1
fi

# ─── Deploy Function ────────────────────────────────────────────────────────
deploy() {
    local admin="${1:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    
    echo "🚀 Storing Airdrop Code..."
    local res=$($BINARY tx wasm store "$WASM_PATH" --from "$KEY_NAME" \
        --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" --chain-id "$CHAIN_ID" \
        --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json)
    
    local code_id=$(echo "$res" | jq -r '.logs[0].events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')
    
    if [[ -z "$code_id" || "$code_id" == "null" ]]; then
        echo "❌ Failed to store code. Result: $res"
        exit 1
    fi
    
    echo "✅ Code Stored! ID: $code_id"
    sleep 5 # Wait for block inclusion

    echo "🎬 Instantiating Airdrop Contract..."
    local inst_msg="{\"admin\":\"$admin\"}"
    
    local inst_res=$($BINARY tx wasm instantiate "$code_id" "$inst_msg" --label "Oroswap Airdrop" \
        --admin "$admin" --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
        --node "$RPC_URL" --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
        --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json)
    
    local contract_addr=$(echo "$inst_res" | jq -r '.logs[0].events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value')
    
    if [[ -z "$contract_addr" || "$contract_addr" == "null" ]]; then
        echo "❌ Failed to instantiate contract. Result: $inst_res"
        exit 1
    fi

    echo "🏁 Airdrop Contract Deployed!"
    echo "📍 Address: $contract_addr"
    echo "💻 Admin: $admin"
}

# ─── Main ──────────────────────────────────────────────────────────────────
deploy "$@"
