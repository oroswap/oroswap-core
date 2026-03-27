#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
# Source environment variables (pointing to devnet or testnet)
source "$(dirname "$0")/../../testnet_testing.env"

WASM_PATH="$(dirname "$0")/../../../opt-artifacts/oroswap_wrapper.wasm"

if [ ! -f "$WASM_PATH" ]; then
    echo "⚠️  Optimized WASM not found at $WASM_PATH. Trying to find it in target/wasm32-unknown-unknown/release/..."
    WASM_PATH="$(dirname "$0")/../../../target/wasm32-unknown-unknown/release/oroswap_wrapper.wasm"
fi

if [ ! -f "$WASM_PATH" ]; then
    echo "❌ Error: WASM file not found. Please build the contract first."
    exit 1
fi

# ─── Deploy Function ────────────────────────────────────────────────────────
deploy() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: deploy <lock_denom> <mint_subdenom> [owner]"; exit 1; fi
    local lock_denom="$1"
    local mint_subdenom="$2"
    local admin="${3:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    
    echo "🚀 1. Storing Wrapper Code..."
    local res=$($BINARY tx wasm store "$WASM_PATH" --from "$KEY_NAME" \
        --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" --chain-id "$CHAIN_ID" \
        --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json)
    
    local code_id=$(echo "$res" | jq -r '.logs[0].events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')
    
    if [[ -z "$code_id" || "$code_id" == "null" ]]; then
        # Try different path for newer ibc-go/wasmd
        code_id=$(echo "$res" | jq -r '.. | select(.type? == "store_code")? | .attributes[] | select(.key == "code_id") | .value' | head -n 1)
    fi

    if [[ -z "$code_id" || "$code_id" == "null" ]]; then
        echo "❌ Failed to retrieve code_id. Result: $res"
        exit 1
    fi
    
    echo "✅ Code Stored! ID: $code_id"
    sleep 5 

    echo "🎬 2. Instantiating Wrapper Contract..."
    # Customize msg based on lock_denom and mint_subdenom
    local inst_msg="{\"lock_denom\":\"$lock_denom\",\"mint_subdenom\":\"$mint_subdenom\",\"owner\":\"$admin\"}"
    
    local inst_res=$($BINARY tx wasm instantiate "$code_id" "$inst_msg" --label "Oroswap Wrapper ($mint_subdenom)" \
        --admin "$admin" --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
        --node "$RPC_URL" --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
        --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json)
    
    local contract_addr=$(echo "$inst_res" | jq -r '.. | select(.type? == "instantiate")? | .attributes[] | select(.key == "_contract_address") | .value' | head -n 1)
    
    if [[ -z "$contract_addr" || "$contract_addr" == "null" ]]; then
        echo "❌ Failed to instantiate contract. Result: $inst_res"
        exit 1
    fi

    echo "🏁 Wrapper Contract Deployed!"
    echo "📍 Address: $contract_addr"
    echo "🪙 Lock Denom: $lock_denom"
    echo "🪙 Mint Subdenom: $mint_subdenom"
    echo "💻 Admin: $admin"

    # Update env file optionally
    # echo "export WRAPPER_CONTRACT=\"$contract_addr\"" >> scripts/testnet_testing.env
}

# ─── Main ──────────────────────────────────────────────────────────────────
deploy "$@"
