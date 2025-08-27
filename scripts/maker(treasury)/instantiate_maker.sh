#!/bin/bash

# Load devnet environment variables
source ../devnet.env

# Get the admin address from key name
ADMIN_ADDRESS=$($BINARY keys show $KEY_NAME --keyring-backend $KEYRING_BACKEND -a)

# Maker contract code ID
#CODE_ID=25

# Instantiate the contract with keeper bridge management support
# critical_tokens: List of tokens that only the owner can manage bridges for
#                  Keepers can only manage bridges for non-critical tokens
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
  "collect_cooldown": 60,
  "critical_tokens": [
    {
      "native_token": {
        "denom": "'$ZIG_ADDRESS'"
      }
    },
    {
      "native_token": {
        "denom": "coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.chat101"
      }
    },
    {
      "native_token": {
        "denom": "coin.zig12jzwc0a3pyv4dze0t252qkwf77t4vs5rqfn3zc.dev3token8"
      }
    }
  ]
}'

# Deploy the contract
$BINARY tx wasm instantiate $MAKER_CODE_ID "$INIT" \
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
$BINARY query wasm list-contract-by-code $MAKER_CODE_ID \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  --output json | jq -r '.contracts[-1]'