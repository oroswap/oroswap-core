#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Query (consult) a deployed Oracle contract for a native-token price
#
# This script sources common settings from devnet.env and then runs a
# `wasm contract-state smart` query to call the Oracle’s `consult` entrypoint.
#
# Usage:
#   1. Make sure that ../devnet.env exports at least:
#        BINARY        # e.g. "zigchaind"
#        RPC_URL       # e.g. "https://devnet-rpc.zigchain.com"
#        CHAIN_ID      # e.g. "zig-devnet-1"
#   2. Edit or set the following variables below:
#        ORACLE_ADDRESS  # the address of your instantiated Oracle contract
#        TOKEN_DENOM     # the native-token denom you want to consult (base unit)
#        AMOUNT          # the amount (in base units) you’re querying for
#   3. Run:
#        chmod +x consult_oracle.sh
#        ./consult_oracle.sh
# -----------------------------------------------------------------------------

source ../devnet.env

# ─── User-configurable settings ──────────────────────────────────────────────
ORACLE_ADDRESS="zig1tlnyy3wctfrdf6g2qaprs47p4acfy6lyvpctn33nlnz27rexhn5q2w58xr"
TOKEN_DENOM="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.ufin1"
AMOUNT="1"
# ──────────────────────────────────────────────────────────────────────────────

# Verify required variables from devnet.env
: "${BINARY:?Must set BINARY in devnet.env (e.g. zigchaind)}"
: "${RPC_URL:?Must set RPC_URL in devnet.env}"
: "${CHAIN_ID:?Must set CHAIN_ID in devnet.env}"

echo "🔹 Using the following parameters:"
echo "   • Binary:          ${BINARY}"
echo "   • RPC URL:         ${RPC_URL}"
echo "   • Chain ID:        ${CHAIN_ID}"
echo "   • Oracle:          ${ORACLE_ADDRESS}"
echo "   • Token denom:     ${TOKEN_DENOM}"
echo "   • Query amount:    ${AMOUNT}"
echo

# Build the JSON payload as a single-quoted string.
# Escaping the inner double quotes so that zigchaind sees valid JSON.
QUERY_JSON="{\"consult\":{\"token\":{\"native_token\":{\"denom\":\"${TOKEN_DENOM}\"}},\"amount\":\"${AMOUNT}\"}}"

echo "🔹 Query JSON:"
echo "  ${QUERY_JSON}"
echo

echo "▶️  Querying Oracle (consult)..."
set +e
RESPONSE=$(${BINARY} query wasm contract-state smart "${ORACLE_ADDRESS}" "${QUERY_JSON}" \
  --node "${RPC_URL}" \
  --chain-id "${CHAIN_ID}" \
  --output json 2>&1)
EXIT_CODE=$?
set -e

if [ "${EXIT_CODE}" -ne 0 ]; then
  echo
  echo "❌ Query failed (exit code ${EXIT_CODE}):"
  echo "${RESPONSE}"
  exit "${EXIT_CODE}"
fi

echo
echo "✅ Oracle response:"
echo "${RESPONSE}"
