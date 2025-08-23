#!/bin/bash
set -euo pipefail

# Source environment variables
source "$(dirname "$0")/../devnet.env"

# Check if pool initializer address is provided
if [[ -z "${POOL_INITIALIZER_ADDR:-}" ]]; then
    echo "❌ POOL_INITIALIZER_ADDR not set. Please run instantiate_pool_initializer.sh first."
    exit 1
fi

echo "🧪 Testing Pool Initializer Contract..."
echo "📍 Contract Address: $POOL_INITIALIZER_ADDR"
echo ""

# Test 1: Query contract configuration
echo "🔍 Test 1: Querying contract configuration..."
CONFIG=$(zigchaind query wasm contract-state smart $POOL_INITIALIZER_ADDR '{"config": {}}' \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  --output json)

echo "✅ Contract Config:"
echo "$CONFIG" | jq '.'
echo ""

# Test 3: Create a XYK_30 pool with native token and ZIG
echo "🔍 Test 3: Creating XYK_30 pool with Native Token/ZIG..."
echo "📝 Creating pool with 100,000 native tokens and 1 ZIG initial liquidity..."
echo "💰 Sending 102 ZIG total (101 ZIG pair creation fee + 1 ZIG liquidity)"
echo "🎯 Using XYK_30 pair type (0.3% fee tier)"
echo "📝 Note: This test uses native tokens only. For CW-20 tokens, users must first approve the pool_initializer contract."

# Native token denom
NATIVE_TOKEN_DENOM="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.poolinit102"

# Note: 102 ZIG total = 101 ZIG for pair creation fee + 1 ZIG for liquidity

zigchaind tx wasm execute $POOL_INITIALIZER_ADDR \
  '{
    "create_pair_and_provide_liquidity": {
      "pair_type": {"custom": "xyk_30"},
      "asset_infos": [
        {"native_token": {"denom": "'$NATIVE_TOKEN_DENOM'"}},
        {"native_token": {"denom": "uzig"}}
      ],
      "init_params": null,
      "liquidity": {
        "assets": [
          {"info": {"native_token": {"denom": "'$NATIVE_TOKEN_DENOM'"}}, "amount": "100000"},
          {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"}
        ],
        "slippage_tolerance": "0.01",
        "receiver": null,
        "min_lp_to_receive": null
      }
    }
  }' \
  --amount "102000000uzig,100000$NATIVE_TOKEN_DENOM" \
  --from $KEY_NAME --keyring-backend $KEYRING_BACKEND \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES \
  -y

echo ""
echo "⏳ Waiting for pool creation transaction to be processed..."
sleep $SLEEP_TIME

echo "✅ Pool creation transaction submitted!"
echo ""
echo "💡 To verify the pool was created:"
echo "   1. Check the transaction logs for pair creation events"
echo "   2. Query the factory for the new pair"
echo "   3. Verify liquidity was added to the pair"
echo ""
echo "🔍 To query factory pairs:"
echo "   zigchaind query wasm contract-state smart $FACTORY_CONTRACT '{\"pairs\": {\"start_after\": null, \"limit\": 10}}' --node $RPC_URL --chain-id $CHAIN_ID"
echo ""
echo "🔍 To query the specific pair:"
echo "   zigchaind query wasm contract-state smart $FACTORY_CONTRACT '{\"pair\": {\"asset_infos\": [{\"native_token\": {\"denom\": \"'$NATIVE_TOKEN_DENOM'\"}}, {\"native_token\": {\"denom\": \"uzig\"}}], \"pair_type\": {\"xyk\": {}}}}' --node $RPC_URL --chain-id $CHAIN_ID"
echo ""
echo "🎉 Pool Initializer test completed!"
