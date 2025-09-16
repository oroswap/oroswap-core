#!/bin/bash
set -euo pipefail

source ../devnet.env

echo "🔄 Updating Factory Pair Configurations..."
echo "Factory Address: $FACTORY_CONTRACT"

# Update pair configurations to add new XYK fee tiers
echo "📝 Adding new XYK fee tiers..."

# Add xyk_1
echo "➕ Adding xyk_1 (1 bps fee)..."
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "update_pair_config": {
    "config": {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "custom": "xyk_1" },
      "permissioned": false,
      "total_fee_bps": 1,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  }
}' \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas-adjustment $GAS_ADJUSTMENT \
  --gas auto \
  --yes \
  --node $RPC_URL \
  --keyring-backend $KEYRING_BACKEND

sleep $SLEEP_TIME

# Add xyk_10
echo "➕ Adding xyk_10 (10 bps fee)..."
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "update_pair_config": {
    "config": {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "custom": "xyk_10" },
      "permissioned": false,
      "total_fee_bps": 10,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  }
}' \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas-adjustment $GAS_ADJUSTMENT \
  --gas auto \
  --yes \
  --node $RPC_URL \
  --keyring-backend $KEYRING_BACKEND

sleep $SLEEP_TIME

# Add xyk_25
echo "➕ Adding xyk_25 (25 bps fee)..."
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "update_pair_config": {
    "config": {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "custom": "xyk_25" },
      "permissioned": false,
      "total_fee_bps": 25,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  }
}' \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas-adjustment $GAS_ADJUSTMENT \
  --gas auto \
  --yes \
  --node $RPC_URL \
  --keyring-backend $KEYRING_BACKEND

sleep $SLEEP_TIME

# Add xyk_100
echo "➕ Adding xyk_100 (100 bps fee)..."
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "update_pair_config": {
    "config": {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "custom": "xyk_100" },
      "permissioned": false,
      "total_fee_bps": 100,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  }
}' \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas-adjustment $GAS_ADJUSTMENT \
  --gas auto \
  --yes \
  --node $RPC_URL \
  --keyring-backend $KEYRING_BACKEND

sleep $SLEEP_TIME

# Add xyk_200
echo "➕ Adding xyk_200 (200 bps fee)..."
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "update_pair_config": {
    "config": {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "custom": "xyk_200" },
      "permissioned": false,
      "total_fee_bps": 200,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  }
}' \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas-adjustment $GAS_ADJUSTMENT \
  --gas auto \
  --yes \
  --node $RPC_URL \
  --keyring-backend $KEYRING_BACKEND

sleep $SLEEP_TIME

echo "✅ Factory pair configurations updated successfully!"
echo "📋 New XYK fee tiers added:"
echo "   - xyk_1 (1 bps = 0.01%)"
echo "   - xyk_10 (10 bps = 0.1%)"
echo "   - xyk_25 (25 bps = 0.25%)"
echo "   - xyk_100 (100 bps = 1%)"
echo "   - xyk_200 (200 bps = 2%)"
echo ""
echo "🔍 You can now create pairs with these new fee tiers!"
