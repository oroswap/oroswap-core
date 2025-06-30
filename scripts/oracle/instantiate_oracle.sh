#!/bin/bash

# -----------------------------------------------------------------------------
# Instantiate an Astroport Oracle for a native-token pair (Asset1 <→> Asset2)
# with an admin address for future migrations.
#
# Oracle-specific settings are defined below. Everything else (chain RPC, key,
# factory address, fees, etc.) is sourced from devnet.env.
#
# Usage:
#   1. Update ORACLE_CODE_ID below if needed.
#   2. Ensure that ../devnet.env exports at least:
#        BINARY
#        RPC_URL
#        CHAIN_ID
#        KEY_NAME
#        KEYRING_BACKEND
#        FACTORY_CONTRACT
#        GAS_ADJUSTMENT
#        FEES
#        SLEEP_TIME
#   3. Run:
#        chmod +x instantiate-oracle.sh
#        ./instantiate-oracle.sh
# -----------------------------------------------------------------------------

# ─── Oracle-specific configuration (edit these) ──────────────────────────────
ORACLE_CODE_ID="12"  # <-- Replace with your deployed Oracle code ID
ASSET1_DENOM="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.ufin1"
ASSET2_DENOM="uzig"
# ──────────────────────────────────────────────────────────────────────────────

# Import shared environment variables
source ../devnet.env

# Verify that shared variables are set
: "${BINARY:?Must set BINARY in devnet.env}"
: "${RPC_URL:?Must set RPC_URL in devnet.env}"
: "${CHAIN_ID:?Must set CHAIN_ID in devnet.env}"
: "${KEY_NAME:?Must set KEY_NAME in devnet.env}"
: "${KEYRING_BACKEND:?Must set KEYRING_BACKEND in devnet.env}"
: "${FACTORY_CONTRACT:?Must set FACTORY_CONTRACT in devnet.env}"
: "${GAS_ADJUSTMENT:?Must set GAS_ADJUSTMENT in devnet.env}"
: "${FEES:?Must set FEES in devnet.env}"
: "${SLEEP_TIME:?Must set SLEEP_TIME in devnet.env}"

# Derive the admin address from the key
ADMIN_ADDRESS=$(${BINARY} keys show "${KEY_NAME}" --keyring-backend "${KEYRING_BACKEND}" -a)

echo "🔹 Using the following parameters:"
echo "   • Chain RPC:            ${RPC_URL}"
echo "   • Chain ID:             ${CHAIN_ID}"
echo "   • Key name:             ${KEY_NAME}"
echo "   • Admin address:        ${ADMIN_ADDRESS}"
echo "   • Factory Address:      ${FACTORY_CONTRACT}"
echo "   • Asset1 Denom:         ${ASSET1_DENOM}"
echo "   • Asset2 Denom:         ${ASSET2_DENOM}"
echo "   • Oracle Code ID:       ${ORACLE_CODE_ID}"
echo

# Build the JSON init message
read -r -d '' INIT_JSON <<EOF
{
  "factory_contract": "${FACTORY_CONTRACT}",
  "asset_infos": [
    { "native_token": { "denom": "${ASSET1_DENOM}" } },
    { "native_token": { "denom": "${ASSET2_DENOM}" } }
  ]
}
EOF

echo "🔹 Init message:"
echo "${INIT_JSON}"
echo

# Instantiate the Oracle contract and capture the transaction hash
echo "▶️  Instantiating Oracle..."
TX_HASH=$(
  ${BINARY} tx wasm instantiate "${ORACLE_CODE_ID}" "${INIT_JSON}" \
    --from "${KEY_NAME}" \
    --chain-id "${CHAIN_ID}" \
    --node "${RPC_URL}" \
    --label "oracle-${ASSET1_DENOM}-${ASSET2_DENOM}" \
    --admin "${ADMIN_ADDRESS}" \
    --gas auto \
    --gas-adjustment "${GAS_ADJUSTMENT}" \
    --fees "${FEES}" \
    -y \
    --output json \
  | jq -r .txhash
)

echo "✅ Tx hash: ${TX_HASH}"
echo
echo "⏳ Waiting ${SLEEP_TIME}s for the transaction to be included..."
sleep "${SLEEP_TIME}"

# Fetch the newly instantiated Oracle contract address from the tx events
ORACLE_ADDRESS=$(
  ${BINARY} query tx "${TX_HASH}" \
    --node "${RPC_URL}" \
    --chain-id "${CHAIN_ID}" \
    --output json \
  | jq -r '
    .logs[0].events[]
    | select(.type=="instantiate")
    | .attributes[]
    | select(.key=="_contract_address")
    | .value
  '
)

echo "🎉 Oracle contract instantiated at: ${ORACLE_ADDRESS}"
