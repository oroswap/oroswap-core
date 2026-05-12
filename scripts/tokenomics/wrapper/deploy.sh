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
    if [[ $# -lt 3 ]]; then
        echo "❌ Usage: deploy <lock_denom> <mint_subdenom> <mint_decimals> [owner]"
        echo "   <mint_decimals> MUST match the source token's decimals so that"
        echo "   wallets display 1:1 wrapping correctly. E.g. 6 for USDC,"
        echo "   18 for most ERC-20s. Baked in once at create-time."
        exit 1
    fi
    local lock_denom="$1"
    local mint_subdenom="$2"
    local mint_decimals="$3"
    local admin="${4:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    
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
    # mint_decimals is required — must match the source token's decimals.
    local inst_msg="{\"lock_denom\":\"$lock_denom\",\"mint_subdenom\":\"$mint_subdenom\",\"mint_decimals\":$mint_decimals,\"owner\":\"$admin\"}"
    
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
    echo "🔢 Mint Decimals: $mint_decimals (immutable; baked into denom metadata)"
    echo "💻 Admin: $admin"

    # Update env file optionally
    # echo "export WRAPPER_CONTRACT=\"$contract_addr\"" >> scripts/testnet_testing.env
}

# ─── Main ──────────────────────────────────────────────────────────────────
deploy "$@"
