#!/usr/bin/env bash
set -euo pipefail

# ─── Load environment variables ─────────────────────────────────────────────────────
source ../devnet.env

# ─── Configuration ──────────────────────────────────────────────────────────────────
BINARY="$BINARY"
RPC_URL="$RPC_URL"
CHAIN_ID="$CHAIN_ID"
KEY_NAME="$KEY_NAME"
KEYRING_BACKEND="$KEYRING_BACKEND"
MAKER_CONTRACT="$MAKER_CONTRACT"

# Transaction settings
TX_FEES="$FEES"
GAS_ADJUSTMENT="$GAS_ADJUSTMENT"

# ─── Functions ──────────────────────────────────────────────────────────────────────

# Set critical tokens (owner only)
set_critical_tokens() {
    echo "🔒 Setting critical tokens (owner only)..."
    $BINARY tx wasm execute "$MAKER_CONTRACT" '{
      "update_config": {
        "critical_tokens": [
          {"native_token": {"denom": "uzig"}},
          {"native_token": {"denom": "usdc"}},
          {"native_token": {"denom": "usdt"}},
          {"token": {"contract_addr": "zig1..."}}
        ]
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Add keeper (owner only)
add_keeper() {
    local keeper_address="$1"
    echo "👤 Adding keeper: $keeper_address"
    $BINARY tx wasm execute "$MAKER_CONTRACT" '{
      "add_keeper": {
        "keeper": "'"$keeper_address"'"
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Keeper adds bridge for non-critical token
keeper_add_bridge() {
    local token_address="$1"
    local bridge_address="$2"
    echo "🔗 Keeper adding bridge: $token_address → $bridge_address"
    $BINARY tx wasm execute "$MAKER_CONTRACT" '{
      "update_bridges": {
        "add": [
          [
            {"token": {"contract_addr": "'"$token_address"'"}},
            {"token": {"contract_addr": "'"$bridge_address"'"}}
          ]
        ]
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Keeper removes bridge for non-critical token
keeper_remove_bridge() {
    local token_address="$1"
    echo "🗑️ Keeper removing bridge for: $token_address"
    $BINARY tx wasm execute "$MAKER_CONTRACT" '{
      "update_bridges": {
        "remove": [
          {"token": {"contract_addr": "'"$token_address"'"}}
        ]
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# Query current config
query_config() {
    echo "📋 Querying maker config..."
    $BINARY query wasm contract-state smart "$MAKER_CONTRACT" '{"config":{}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Query current bridges
query_bridges() {
    echo "🌉 Querying current bridges..."
    $BINARY query wasm contract-state smart "$MAKER_CONTRACT" '{"bridges":{}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
}

# Test keeper trying to modify critical token (should fail)
test_keeper_critical_token_fail() {
    echo "❌ Testing keeper trying to modify critical token (should fail)..."
    $BINARY tx wasm execute "$MAKER_CONTRACT" '{
      "update_bridges": {
        "add": [
          [
            {"native_token": {"denom": "uzig"}},
            {"native_token": {"denom": "usdc"}}
          ]
        ]
      }
    }' \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    -y -o json | jq
}

# ─── Usage Examples ────────────────────────────────────────────────────────────────

echo "🚀 Maker Keeper Bridge Management Examples"
echo "=========================================="

case "${1:-}" in
    "setup")
        echo "🔧 Setting up critical tokens and keeper..."
        set_critical_tokens
        sleep 3
        add_keeper "zig1keeperaddress..."
        ;;
    "keeper-add")
        if [ -z "$2" ] || [ -z "$3" ]; then
            echo "Usage: $0 keeper-add <token_address> <bridge_address>"
            exit 1
        fi
        keeper_add_bridge "$2" "$3"
        ;;
    "keeper-remove")
        if [ -z "$2" ]; then
            echo "Usage: $0 keeper-remove <token_address>"
            exit 1
        fi
        keeper_remove_bridge "$2"
        ;;
    "query-config")
        query_config
        ;;
    "query-bridges")
        query_bridges
        ;;
    "test-fail")
        test_keeper_critical_token_fail
        ;;
    *)
        echo "Usage: $0 {setup|keeper-add|keeper-remove|query-config|query-bridges|test-fail}"
        echo ""
        echo "Examples:"
        echo "  $0 setup                                    # Set up critical tokens and keeper"
        echo "  $0 keeper-add zig1token... zig1bridge...    # Keeper adds bridge"
        echo "  $0 keeper-remove zig1token...               # Keeper removes bridge"
        echo "  $0 query-config                             # Query current config"
        echo "  $0 query-bridges                            # Query current bridges"
        echo "  $0 test-fail                                # Test keeper modifying critical token"
        exit 1
        ;;
esac
