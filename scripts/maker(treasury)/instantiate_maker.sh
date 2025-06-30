#!/bin/bash

# Load devnet environment variables
source ../devnet.env

# Get the admin address from key name
ADMIN_ADDRESS=$($BINARY keys show $KEY_NAME --keyring-backend $KEYRING_BACKEND -a)

# Maker contract code ID
CODE_ID=151

# Instantiate the contract
INIT='{
  "owner": "'$ADMIN_ADDRESS'",
  "oro_token": {
    "native_token": {
      "denom": "'$ZIG_ADDRESS'"
    }
  },
  "factory_contract": "'$FACTORY_CONTRACT'",
  "staking_contract": null,
  "governance_contract": "'$ADMIN_ADDRESS'",
  "governance_percent": "100",
  "max_spread": "0.05",
  "collect_cooldown": 60
}'

# Deploy the contract
$BINARY tx wasm instantiate $CODE_ID "$INIT" \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas auto \
  --gas-adjustment $GAS_ADJUSTMENT \
  --label "Oroswap Maker" \
  --admin $ADMIN_ADDRESS \
  --keyring-backend $KEYRING_BACKEND \
  --node $RPC_URL \
  -y

# Sleep to allow transaction to be processed
sleep $SLEEP_TIME

# Query the contract address
$BINARY query wasm list-contract-by-code $CODE_ID \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  --output json | jq -r '.contracts[-1]'