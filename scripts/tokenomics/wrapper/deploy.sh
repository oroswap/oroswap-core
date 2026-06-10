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
    if [[ $# -lt 4 ]]; then
        echo "❌ Usage: deploy <lock_denom> <mint_subdenom> <mint_decimals> <mint_max_supply> [owner] [mint_uri] [mint_uri_hash]"
        echo "   <mint_decimals>    MUST match the source token's decimals so that"
        echo "                      wallets display 1:1 wrapping correctly. E.g. 6 for USDC."
        echo "   <mint_max_supply>  FIND-005: wrapper-enforced cap on the CIRCULATING"
        echo "                      supply of the minted denom, in BASE units (i.e."
        echo "                      already × 10^mint_decimals). Burns free headroom,"
        echo "                      so wrap/unwrap cycles work forever as long as"
        echo "                      instantaneous supply stays under this value."
        echo "                      Example: 790,000,000 ORO @ 6 decimals → 790000000000000."
        echo "   [mint_uri]         Optional logo / metadata URI."
        echo "   [mint_uri_hash]    Optional integrity hash for mint_uri. MUST be a"
        echo "                      64-character alphanumeric SHA256 hex (ZIGchain"
        echo "                      factory module rejects anything shorter or with"
        echo "                      separators like 'sha256:'). Editable post-deploy."
        echo ""
        echo "   The instantiate call attaches --amount 1000uzig to fund the contract"
        echo "   for the ZigChain factory MsgCreateDenom fee (~1000uzig)."
        exit 1
    fi
    local lock_denom="$1"
    local mint_subdenom="$2"
    local mint_decimals="$3"
    local mint_max_supply="$4"
    local admin="${5:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    local mint_uri="${6:-}"
    local mint_uri_hash="${7:-}"

    # Pre-flight: refuse a zero cap before paying for the store-code tx.
    # The contract also rejects this at instantiate, but a CLI-side guard
    # saves an unnecessary trip on chain.
    if [[ "$mint_max_supply" == "0" ]]; then
        echo "❌ mint_max_supply must be > 0 (contract will reject with InvalidMintMaxSupply)."
        exit 1
    fi
    
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
    # Schema requirements:
    #   - mint_decimals must match the source token's decimals.
    #   - mint_max_supply is required (FIND-005) — base units, > 0,
    #     enforced wrapper-side against live circulating supply.
    #
    # Funding requirement (D-1):
    #   --amount 1000uzig is REQUIRED. The wrapper creates its tokenfactory
    #   denom in its `instantiate` reply, and the ZigChain factory module
    #   charges 1000uzig for `MsgCreateDenom` from the CONTRACT's own
    #   account. Without --amount the contract has zero balance at
    #   instantiate time → the sub-message reverts → the whole tx
    #   reverts. Don't trim this.
    # mint_uri / mint_uri_hash are Option<String> in InstantiateMsg — omit
    # the key entirely when caller didn't supply (Option defaults to None).
    local extra=""
    [[ -n "$mint_uri" ]]      && extra+=",\"mint_uri\":\"$mint_uri\""
    [[ -n "$mint_uri_hash" ]] && extra+=",\"mint_uri_hash\":\"$mint_uri_hash\""
    local inst_msg="{\"lock_denom\":\"$lock_denom\",\"mint_subdenom\":\"$mint_subdenom\",\"mint_decimals\":$mint_decimals,\"mint_max_supply\":\"$mint_max_supply\",\"owner\":\"$admin\"$extra}"

    local inst_res=$($BINARY tx wasm instantiate "$code_id" "$inst_msg" --label "Oroswap Wrapper ($mint_subdenom)" \
        --admin "$admin" --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" --amount 1000uzig \
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
    echo "🧢 Mint Max Supply: $mint_max_supply base units (immutable on-chain)"
    echo "💻 Admin: $admin"

    # Update env file optionally
    # echo "export WRAPPER_CONTRACT=\"$contract_addr\"" >> scripts/testnet_testing.env
}

# ─── Main ──────────────────────────────────────────────────────────────────
deploy "$@"
