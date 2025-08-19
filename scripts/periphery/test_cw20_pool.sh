#!/usr/bin/env bash
set -euo pipefail

# Source environment variables
source scripts/devnet.env

# CW-20 token address (from the token we just created)
CW20_TOKEN1="zig18d35q9k6wrwa990qjehrj28tyjc9qgmrtw5vypkc8ljs8nch0lyqskqs3v"
echo "Using CW-20 token: $CW20_TOKEN1"
echo "Using native token: uzig"

# Test 1: Create pool with two CW-20 tokens
echo "Testing CW-20 pool creation..."

# First, approve the pool initializer to spend CW-20 tokens
echo "Approving CW-20 token for pool initializer..."

# Approve token1 (CW-20)
zigchaind tx wasm execute "$CW20_TOKEN1" "{\"increase_allowance\": {\"spender\": \"$POOL_INITIALIZER_ADDR\", \"amount\": \"1000\"}}" \
  --from "$KEY_NAME" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id "$CHAIN_ID" \
  --node "$RPC_URL" \
  --gas auto \
  --gas-adjustment 1.4 \
  --fees 2000uzig \
  -y

sleep 3

# Create the pool with CW-20 and native tokens
POOL_MSG=$(cat <<EOF
{
  "create_pair_and_provide_liquidity": {
    "pair_type": {"custom": "xyk_30"},
    "asset_infos": [
      {
        "token": {
          "contract_addr": "$CW20_TOKEN1"
        }
      },
      {
        "native_token": {
          "denom": "uzig"
        }
      }
    ],
    "liquidity": {
      "assets": [
        {
          "info": {
            "token": {
              "contract_addr": "$CW20_TOKEN1"
            }
          },
          "amount": "1000"
        },
        {
          "info": {
            "native_token": {
              "denom": "uzig"
            }
          },
          "amount": "1000000"
        }
      ],
      "slippage_tolerance": "0.01",
      "auto_stake": false
    }
  }
}
EOF
)

echo "Creating CW-20 pool with message:"
echo "$POOL_MSG"

POOL_TX=$(zigchaind tx wasm execute "$POOL_INITIALIZER_ADDR" "$POOL_MSG" \
  --from "$KEY_NAME" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id "$CHAIN_ID" \
  --node "$RPC_URL" \
  --gas auto \
  --gas-adjustment 1.4 \
  --fees 50000uzig \
  --amount 102000000uzig \
  -y \
  --output json | jq -r '.txhash')

echo "Pool creation transaction: $POOL_TX"

# Wait for transaction to be processed
sleep 5

# Check the transaction result
echo "Checking transaction result..."
zigchaind query tx "$POOL_TX" --node "$RPC_URL" --chain-id "$CHAIN_ID" --output json | jq '.tx_response.code'

echo "✅ CW-20 + Native pool creation test completed!"
echo "Pool transaction: $POOL_TX"
echo "CW-20 Token: $CW20_TOKEN1"
echo "Native Token: uzig"
