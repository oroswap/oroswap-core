#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------------
# Script: coin_registry.sh
# Purpose: Deploy and manage the Astroport Native Coin Registry on Devnet
# Usage: coin_registry.sh <command> [args]
# Commands:
#   instantiate
#   link-factory
#   add-coin <DENOM> <DECIMALS>
#   register-coin <DENOM> <DECIMALS> <AMOUNT>
#   fetch-coins
#   check-coin <DENOM>              # Check if a specific coin is registered
# ------------------------------------------------------------------

# ─── Load Devnet environment variables ────────────────────────────────────────
# Expecting devnet.env to define:
#   BINARY, RPC_URL, CHAIN_ID, KEY_NAME, KEYRING_BACKEND, FACTORY_CONTRACT, COIN_REGISTRY_ADDR
source ../devnet.env


ADMIN_WALLET=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

FEES="1000uzig"

# ─── Instantiate the native coin registry ─────────────────────────────────────
instantiate() {
  echo "Instantiating oroswap Native Coin Registry (code_id=$COIN_REGISTRY_CODE_ID)..."
  TX=$(
    $BINARY tx wasm instantiate "$COIN_REGISTRY_CODE_ID" '{
      "owner": "'"$ADMIN_WALLET"'"
    }' \
      --label "devnet-coin-registry" \
      --admin "$ADMIN_WALLET" \
      --from "$KEY_NAME" \
      --chain-id "$CHAIN_ID" \
      --node "$RPC_URL" \
      --gas auto \
      --gas-adjustment 1.3 \
      --fees "$FEES" \
      -y -o json \
    | jq -r .txhash
  )
  echo "  → txhash: $TX"
  echo "  Waiting a few seconds for the instantiate to finalize..."
  sleep 5
  echo "✅ Native coin registry instantiated. Update COIN_REGISTRY_ADDR in devnet.env if needed."
}

# ─── Link the factory to use this coin registry ──────────────────────────────
link-factory() {
  echo "Linking factory ($FACTORY_CONTRACT) to coin registry ($COIN_REGISTRY_ADDR)..."
  $BINARY tx wasm execute "$FACTORY_CONTRACT" '{
    "update_config": {
      "coin_registry_address": "'"$COIN_REGISTRY_ADDR"'"
    }
  }' \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment 1.3 \
    --fees "$FEES" \
    -y -o json | jq
  echo "✅ Factory now references the native coin registry."
}

# ─── Add or update a native coin's precision in the registry (ADMIN‐ONLY) ────
# Usage: add-coin <DENOM> <DECIMALS>
add-coin() {
  if [[ $# -ne 2 ]]; then
    echo "Usage: $0 add-coin <DENOM> <DECIMALS>"
    exit 1
  fi
  local DENOM="$1"
  local DECIMALS="$2"
  echo "Adding/updating $DENOM with $DECIMALS decimals in the registry ($COIN_REGISTRY_ADDR)..."
  $BINARY tx wasm execute "$COIN_REGISTRY_ADDR" '{
    "add": {
      "native_coins": [
        ["'"$DENOM"'", '"$DECIMALS"']
      ]
    }
  }' \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment 1.3 \
    --fees "$FEES" \
    -y -o json | jq
  echo "✅ $DENOM precision set to $DECIMALS."
}

# ─── Register a coin by NON‐ADMIN (send a small amount) ───────────────────────
# Usage: register-coin <DENOM> <DECIMALS> <AMOUNT>
register-coin() {
  if [[ $# -ne 3 ]]; then
    echo "Usage: $0 register-coin <DENOM> <DECIMALS> <AMOUNT>"
    exit 1
  fi
  local DENOM="$1"
  local DECIMALS="$2"
  local AMOUNT="$3"
  echo "Registering native coin $DENOM with $DECIMALS decimals (sending $AMOUNT $DENOM) in registry ($COIN_REGISTRY_ADDR)..."
  $BINARY tx wasm execute "$COIN_REGISTRY_ADDR" '{
    "register": {
      "native_coins": [
        ["'"$DENOM"'", '"$DECIMALS"']
      ]
    }
  }' \
    --amount "$AMOUNT$DENOM" \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment 1.3 \
    --fees "$FEES" \
    -y -o json | jq
  echo "✅ Permissionless registration for $DENOM submitted."
}

# ─── Check if a specific coin is registered ─────────────────────────────────
# Usage: check-coin <DENOM>
check-coin() {
  if [[ $# -ne 1 ]]; then
    echo "Usage: $0 check-coin <DENOM>"
    exit 1
  fi
  local DENOM="$1"
  echo "Checking if $DENOM is registered in the coin registry ($COIN_REGISTRY_ADDR)..."
  $BINARY query wasm contract-state smart "$COIN_REGISTRY_ADDR" '{"native_token":{"denom":"'"$DENOM"'"}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
  echo "✅ Done."
}

# ─── Fetch (query) all coins in the registry ─────────────────────────────────
# Usage: fetch-coins
#
# The contract's QueryMsg uses `native_tokens` to list every registered entry.
fetch-coins() {
  echo "Fetching all registered coins from ($COIN_REGISTRY_ADDR) via `native_tokens` query..."
  $BINARY query wasm contract-state smart "$COIN_REGISTRY_ADDR" '{"native_tokens":{}}' \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json | jq
  echo "✅ Done."
}

# ─── Usage helper ─────────────────────────────────────────────────────────────
usage() {
  cat <<EOF
Usage: $0 <command> [args]
Commands:
  instantiate                         # Instantiate the native coin registry (code_id=14)
  link-factory                        # Update the factory to point at this registry
  add-coin <DENOM> <DECIMALS>         # (ADMIN only) Register or update a native coin’s precision
  register-coin <DENOM> <DECIMALS> <AMOUNT>
                                       # (ANYONE) Register coin by sending <AMOUNT><DENOM>
  check-coin <DENOM>                  # Check if a specific coin is registered
  fetch-coins                         # Query and list all registered coins
EOF
  exit 1
}

# ─── Dispatch command ─────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  usage
fi

CMD="$1"; shift
case "$CMD" in
  instantiate)      instantiate ;;
  link-factory)     link-factory ;;
  add-coin)         add-coin "$@" ;;
  register-coin)    register-coin "$@" ;;
  check-coin)       check-coin "$@" ;;
  fetch-coins)      fetch-coins ;;
  *)                usage ;;
esac
