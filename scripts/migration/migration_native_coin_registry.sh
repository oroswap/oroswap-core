#!/bin/bash

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Change to project root directory
cd "$PROJECT_ROOT"

echo "📁 Working directory: $(pwd)"
echo "📁 Script directory: $SCRIPT_DIR"
echo "📁 Project root: $PROJECT_ROOT"

# Source environment variables
source "devnet.env"

# Verify environment variables are loaded
if [ -z "$BINARY" ]; then
    echo "❌ Error: BINARY variable not set. Please check scripts/devnet.env"
    exit 1
fi

if [ -z "$COIN_REGISTRY_ADDR" ]; then
    echo "❌ Error: COIN_REGISTRY_ADDR variable not set. Please check scripts/devnet.env"
    exit 1
fi

if [ -z "$COIN_REGISTRY_CODE_ID" ]; then
    echo "❌ Error: COIN_REGISTRY_CODE_ID variable not set. Please check scripts/devnet.env"
    exit 1
fi

echo "🔄 Migrating Native Coin Registry Contract..."
echo "Contract Address: $COIN_REGISTRY_ADDR"
echo "New Code ID: $COIN_REGISTRY_CODE_ID"
echo "Chain ID: $CHAIN_ID"
echo ""

# Migration message (empty for now, can be customized if needed)
MIGRATE_MSG='{}'

echo "📝 Migration Message: $MIGRATE_MSG"
echo ""

# Execute migration
echo "🚀 Executing migration..."
$BINARY tx wasm migrate $COIN_REGISTRY_ADDR $COIN_REGISTRY_CODE_ID "$MIGRATE_MSG" \
    --from $KEY_NAME \
    --chain-id $CHAIN_ID \
    --gas-prices $GAS_PRICES \
    --gas-adjustment $GAS_ADJUSTMENT \
    --gas auto \
    --yes \
    --node $RPC_URL \
    --keyring-backend $KEYRING_BACKEND

echo ""
echo "✅ Migration transaction submitted!"
echo "⏳ Waiting $SLEEP_TIME seconds for transaction to be processed..."
sleep $SLEEP_TIME

# Query the contract to verify migration
echo "🔍 Verifying migration..."
$BINARY query wasm contract $COIN_REGISTRY_ADDR \
    --node $RPC_URL \
    --chain-id $CHAIN_ID

echo ""
echo "🎉 Native Coin Registry migration completed!"
echo "Contract Address: $COIN_REGISTRY_ADDR"
echo "New Code ID: $COIN_REGISTRY_CODE_ID"
