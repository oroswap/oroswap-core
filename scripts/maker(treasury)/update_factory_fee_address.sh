#!/bin/bash

# Load devnet environment variables
source ../devnet.env

# Get the admin address from key name
ADMIN_ADDRESS=$($BINARY keys show $KEY_NAME --keyring-backend $KEYRING_BACKEND -a)

# Factory contract address
FACTORY_CONTRACT="zig1wn625s4jcmvk0szpl85rj5azkfc6suyvf75q6vrddscjdphtve8sh9354q"

# New Maker contract address
NEW_MAKER_ADDRESS="zig1e8vp80sdczunxv00rlusu7lmmers0tg0tmfjejwl6n3ad8etk00qm2u0nw"

# Update factory fee address
UPDATE_MSG='{
  "update_config": {
    "fee_address": "'$NEW_MAKER_ADDRESS'"
  }
}'

echo "Updating factory fee address from old Maker to new Maker..."
echo "Old Maker: zig1fplyf3xrtfvcqjew4hnln3a3y4syzltrkjuggscq60v3hudyzhdqtrhu8d"
echo "New Maker: $NEW_MAKER_ADDRESS"

# Execute the update
$BINARY tx wasm execute "$FACTORY_CONTRACT" "$UPDATE_MSG" \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas auto \
  --gas-adjustment $GAS_ADJUSTMENT \
  --keyring-backend $KEYRING_BACKEND \
  --node $RPC_URL \
  -y

# Sleep to allow transaction to be processed
sleep $SLEEP_TIME

echo "Factory fee address updated successfully!"
echo "Querying updated factory config..."

# Query the updated factory config to confirm
$BINARY query wasm contract-state smart "$FACTORY_CONTRACT" '{"config":{}}' \
  --node $RPC_URL \
  --chain-id $CHAIN_ID \
  -o json | jq '.data.fee_address' 