#!/usr/bin/env bash
set -euo pipefail

# Update these values
CHAIN_ID="zig-devnet-1"
NODE="https://devnet-rpc.zigchain.com"
KEY_NAME="devnet-key"
KEYRING_BACKEND="test"
CW20_CODE_ID=${CW20_CODE_ID:-9}
TOKEN_NAME="CW SAGE"
TOKEN_SYMBOL="CWSAGE"
TOKEN_DECIMALS=6
INITIAL_SUPPLY="1000000000000"  # 1,000,000 ORO with 6 decimals
ADMIN_ADDRESS=$(zigchaind keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

# Prepare instantiation message
INIT_MSG=$(cat <<EOF
{
  "name": "$TOKEN_NAME",
  "symbol": "$TOKEN_SYMBOL",
  "decimals": $TOKEN_DECIMALS,
  "initial_balances": [
    {
      "address": "$ADMIN_ADDRESS",
      "amount": "$INITIAL_SUPPLY"
    }
  ],
  "mint": {
    "minter": "$ADMIN_ADDRESS"
  }
}
EOF
)

zigchaind tx wasm instantiate "$CW20_CODE_ID" "$INIT_MSG" \
  --from "$KEY_NAME" \
  --keyring-backend "$KEYRING_BACKEND" \
  --label "$TOKEN_SYMBOL-token" \
  --admin "$ADMIN_ADDRESS" \
  --chain-id "$CHAIN_ID" \
  --node "$NODE" \
  --gas auto \
  --gas-adjustment 1.4 \
  --fees 5000uzig \
  -y \
  --output json
