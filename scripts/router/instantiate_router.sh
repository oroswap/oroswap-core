#!/usr/bin/env bash
set -euo pipefail

# Load Devnet environment variables
source ../devnet.env

ADMIN_ADDRESS=$($BINARY keys show $KEY_NAME --keyring-backend $KEYRING_BACKEND -a)

# Ensure required variables are set
: "${BINARY:?BINARY must be set in devnet.env}"
: "${RPC_URL:?RPC_URL must be set in devnet.env}"
: "${CHAIN_ID:?CHAIN_ID must be set in devnet.env}"
: "${KEY_NAME:?KEY_NAME must be set in devnet.env}"
: "${KEYRING_BACKEND:?KEYRING_BACKEND must be set in devnet.env}"
: "${FACTORY_CONTRACT:?FACTORY_CONTRACT must be set in devnet.env}"
: "${GAS_PRICES:?GAS_PRICES must be set in devnet.env}"
: "${SLEEP_TIME:?SLEEP_TIME must be set in devnet.env}"

# Code ID for the route contract on devnet
ROUTE_CODE_ID=52

# Build the instantiate message with the correct field name
INSTANTIATE_MSG=$(cat <<EOF
{"oroswap_factory":"$FACTORY_CONTRACT"}
EOF
)

echo "Instantiating Route contract (code_id=$ROUTE_CODE_ID) with oroswap_factory=$FACTORY_CONTRACT..."

# Submit the instantiate transaction with sync broadcast mode
TX_RES=$($BINARY tx wasm instantiate "$ROUTE_CODE_ID" "$INSTANTIATE_MSG" \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --gas-prices $GAS_PRICES \
  --gas auto \
  --gas-adjustment $GAS_ADJUSTMENT \
  --label "Oroswap Router" \
  --admin $ADMIN_ADDRESS \
  --keyring-backend $KEYRING_BACKEND \
  --node $RPC_URL \
  -y)

echo "Instantiation response:"
echo "$TX_RES"
