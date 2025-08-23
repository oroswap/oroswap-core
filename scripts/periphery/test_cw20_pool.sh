#!/usr/bin/env bash
set -euo pipefail

# Source environment variables
source "$(dirname "$0")/../devnet.env"

# CW-20 token address (from the token we just created)
CW20_TOKEN1="zig1y55tv3wq40wc3nmlnq25qgm7e5f9cu8ft4fe5vw5m4v7xme0a06s99e2uq"
echo "Using CW-20 token: $CW20_TOKEN1"
echo "Using native token: uzig"

# Test 1: Create pool with two CW-20 tokens
echo "Testing CW-20 pool creation..."

# First, approve the pool initializer to spend CW-20 tokens
echo "Approving CW-20 token for pool initializer..."

# Approve token1 (CW-20) - use a larger amount to be safe
zigchaind tx wasm execute "$CW20_TOKEN1" "{\"increase_allowance\": {\"spender\": \"$POOL_INITIALIZER_ADDR\", \"amount\": \"10000\"}}" \
  --from "$KEY_NAME" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id "$CHAIN_ID" \
  --node "$RPC_URL" \
  --gas auto \
  --gas-adjustment 1.4 \
  --fees 2000uzig \
  -y

sleep 3

# Check the allowance to make sure it was set correctly
echo "Checking allowance..."
zigchaind query wasm contract-state smart "$CW20_TOKEN1" "{\"allowance\": {\"owner\": \"$(zigchaind keys show $KEY_NAME -a --keyring-backend $KEYRING_BACKEND)\", \"spender\": \"$POOL_INITIALIZER_ADDR\"}}" \
  --node "$RPC_URL" \
  --chain-id "$CHAIN_ID" --output json | jq '.data'

sleep 2

# Create the pool with CW-20 and native tokens
POOL_MSG=$(cat <<EOF
{
  "create_pair_and_provide_liquidity": {
    "pair_type": {"custom": "xyk_100"},
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
      "slippage_tolerance": "0.01"
    }
  }
}
EOF
)

echo "Creating CW-20 pool with message:"
echo "$POOL_MSG"
echo ""
echo "Note: The pool initializer will now check CW-20 allowances before proceeding."
echo "If allowance is insufficient, it will fail with a clear error message."

# Execute the transaction and capture the output
echo "Executing pool creation transaction..."
POOL_OUTPUT=$(zigchaind tx wasm execute "$POOL_INITIALIZER_ADDR" "$POOL_MSG" \
  --from "$KEY_NAME" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id "$CHAIN_ID" \
  --node "$RPC_URL" \
  --gas auto \
  --gas-adjustment 1.4 \
  --fees 50000uzig \
  --amount 102000000uzig \
  -y \
  --output json 2>&1)

# Extract transaction hash even if it fails
POOL_TX=$(echo "$POOL_OUTPUT" | grep -o '"txhash":"[^"]*"' | cut -d'"' -f4)

echo "Pool creation transaction: $POOL_TX"
echo "Full transaction output:"
echo "$POOL_OUTPUT"

# Wait for transaction to be processed
echo "Waiting for transaction to be processed..."
sleep 5

# Check the transaction result
echo "Checking transaction result..."
TX_RESULT=$(zigchaind query tx "$POOL_TX" --node "$RPC_URL" --chain-id "$CHAIN_ID" --output json 2>/dev/null || echo "Transaction not found yet")

if echo "$TX_RESULT" | jq -e '.tx_response.code' >/dev/null 2>&1; then
    TX_CODE=$(echo "$TX_RESULT" | jq -r '.tx_response.code')
    if [ "$TX_CODE" = "0" ]; then
        echo "✅ Transaction successful! (code: $TX_CODE)"
    else
        echo "❌ Transaction failed! (code: $TX_CODE)"
        echo "Error details:"
        echo "$TX_RESULT" | jq '.tx_response.raw_log'
    fi
else
    echo "⏳ Transaction still processing or not found yet..."
    echo "You can check manually with: zigchaind query tx $POOL_TX --node $RPC_URL --chain-id $CHAIN_ID"
fi

echo ""
echo "✅ CW-20 + Native pool creation test completed!"
echo "Pool transaction: $POOL_TX"
echo "CW-20 Token: $CW20_TOKEN1"
echo "Native Token: uzig"
echo "Pair Type: xyk_100"
