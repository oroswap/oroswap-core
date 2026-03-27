#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="/Users/cgs/go/bin/zigchaind"
source "$(dirname "$0")/../../devnet_testing.env"
# Overwrite BINARY from env if needed
BINARY="/Users/cgs/go/bin/zigchaind"

WASM_PATH="$(dirname "$0")/../../../opt-artifacts/oroswap_airdrop.wasm"

if [ ! -f "$WASM_PATH" ]; then
    echo "❌ Optimized WASM not found at $WASM_PATH. Please run build_single_contract.sh first."
    exit 1
fi

# ─── Step 1: Deploy (Store Code) ─────────────────────────────────────────────
echo "🚀 1. Storing Airdrop Code..."
STORE_RES=$($BINARY tx wasm store "$WASM_PATH" --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json)

TX_HASH=$(echo "$STORE_RES" | jq -r '.txhash')
echo "✅ Broadcasted! Hash: $TX_HASH"
echo "⏳ Waiting for block inclusion..."

CODE_ID=""
for i in {1..15}; do
    sleep 2
    RESULT=$($BINARY query tx "$TX_HASH" --node "$RPC_URL" -o json 2>/dev/null || echo "{}")
    # Try finding in logs or top-level events
    CODE_ID=$(echo "$RESULT" | jq -r '(.. | select(.type? == "store_code")? | .attributes[] | select(.key == "code_id") | .value) // empty' | head -n 1)
    if [[ -n "$CODE_ID" && "$CODE_ID" != "null" ]]; then
        break
    fi
    echo "..."
done

if [[ -z "$CODE_ID" || "$CODE_ID" == "null" ]]; then
    echo "❌ Failed to retrieve code_id. Last Result: $RESULT"
    exit 1
fi

echo "✅ Code Stored! ID: $CODE_ID"

# ─── Step 2: Wait 10 seconds ─────────────────────────────────────────────────
echo "⏳ 2. Waiting 10 seconds before initialization (as requested)..."
sleep 10

# ─── Step 3: Init (Instantiate) ──────────────────────────────────────────────
echo "🎬 3. Instantiating Airdrop Contract..."
ADMIN_ADDR=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")
INST_MSG="{\"admin\":\"$ADMIN_ADDR\"}"

INST_RES=$($BINARY tx wasm instantiate "$CODE_ID" "$INST_MSG" --label "Oroswap Airdrop" \
    --admin "$ADMIN_ADDR" --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y -o json)

TX_HASH=$(echo "$INST_RES" | jq -r '.txhash')
echo "✅ Broadcasted! Hash: $TX_HASH"
echo "⏳ Waiting for block inclusion..."

CONTRACT_ADDR=""
for i in {1..15}; do
    sleep 2
    RESULT=$($BINARY query tx "$TX_HASH" --node "$RPC_URL" -o json 2>/dev/null || echo "{}")
    # Try finding in logs or top-level events
    CONTRACT_ADDR=$(echo "$RESULT" | jq -r '(.. | select(.type? == "instantiate")? | .attributes[] | select(.key == "_contract_address") | .value) // empty' | head -n 1)
    if [[ -n "$CONTRACT_ADDR" && "$CONTRACT_ADDR" != "null" ]]; then
        break
    fi
    echo "..."
done

if [[ -z "$CONTRACT_ADDR" || "$CONTRACT_ADDR" == "null" ]]; then
    echo "❌ Failed to retrieve contract address. Last Result: $RESULT"
    exit 1
fi

echo "🏁 Airdrop Contract Deployed and Initialized!"
echo "📍 Address: $CONTRACT_ADDR"
echo "💻 Admin: $ADMIN_ADDR"

# Update env file
echo "export AIRDROP_CONTRACT=\"$CONTRACT_ADDR\"" >> ../../devnet_testing.env
