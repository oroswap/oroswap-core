#!/bin/bash
set -euo pipefail

# Source environment variables
source "$(dirname "$0")/../devnet.env"

echo "🚀 Instantiating Pool Initializer Contract..."
echo "📋 Contract Details:"
echo "   - Code ID: $OROSWAP_POOL_INITIALIZER_CODE_ID"
echo "   - Factory Address: $FACTORY_CONTRACT"
echo "   - Chain ID: $CHAIN_ID"
echo ""

    # Instantiate the pool initializer contract
    zigchaind tx wasm instantiate $OROSWAP_POOL_INITIALIZER_CODE_ID \
      "{\"factory_addr\": \"$FACTORY_CONTRACT\", \"pair_creation_fee\": \"101000000\", \"fee_denom\": \"uzig\"}" \
      --label "pool-initializer" \
  --admin $(zigchaind keys show $KEY_NAME -a --keyring-backend $KEYRING_BACKEND) \
  --from $KEY_NAME --keyring-backend $KEYRING_BACKEND \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES \
  -y

echo ""
echo "⏳ Waiting for transaction to be processed..."
sleep $SLEEP_TIME

# Get the contract address
POOL_INITIALIZER_ADDR=$(zigchaind query wasm list-contract-by-code $OROSWAP_POOL_INITIALIZER_CODE_ID \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  --output json | jq -r '.contracts[-1]')

echo ""
echo "✅ Pool Initializer Contract Instantiated Successfully!"
echo "📍 Contract Address: $POOL_INITIALIZER_ADDR"
echo ""
echo "💡 Next Steps:"
echo "   1. Test the contract with a simple pool creation"
echo "   2. Verify the contract configuration"
echo "   3. Test with different asset types"
echo ""
echo "🔍 To query contract config:"
echo "   zigchaind query wasm contract-state smart $POOL_INITIALIZER_ADDR '{\"config\": {}}' --node $RPC_URL --chain-id $CHAIN_ID"
echo ""
echo "📝 To create a pool with liquidity:"
echo "   zigchaind tx wasm execute $POOL_INITIALIZER_ADDR '{\"create_pair_and_provide_liquidity\": {\"pair_type\": {\"xyk\": {}}, \"asset_infos\": [{\"native_token\": {\"denom\": \"uzig\"}}, {\"native_token\": {\"denom\": \"usdc\"}}], \"init_params\": null, \"liquidity\": {\"assets\": [{\"info\": {\"native_token\": {\"denom\": \"uzig\"}}, \"amount\": \"1000000\"}, {\"info\": {\"native_token\": {\"denom\": \"usdc\"}}, \"amount\": \"1000000\"}], \"slippage_tolerance\": \"0.01\", \"auto_stake\": false, \"receiver\": null, \"min_lp_to_receive\": null}}}' --amount \"2000000uzig,1000000usdc\" --from $KEY_NAME --keyring-backend $KEYRING_BACKEND --node $RPC_URL --chain-id $CHAIN_ID --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES -y"
