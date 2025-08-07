#!/usr/bin/env bash
set -euo pipefail

# ─── Load your env (devnet.env) ───────────────────────────────────────────────
source ../devnet.env

# ─── Maker contract address from devnet.env ───────────────────────────────────
# MAKER_CONTRACT is already defined in devnet.env

echo "🧪 Testing Keeper Management for Maker Contract"
echo "Contract: $MAKER_CONTRACT"
echo ""

# ─── Add Keeper Function ─────────────────────────────────────────────────────
add_keeper() {
  if [[ $# -lt 1 ]]; then
    echo "Usage: $0 add_keeper <keeper_address>"
    echo "Example: $0 add_keeper zig1your_wallet_address_here"
    exit 1
  fi
  local keeper_addr=$1

  echo "➕ Adding keeper: $keeper_addr"
  echo ""

  $BINARY tx wasm execute "$MAKER_CONTRACT" "{
    \"add_keeper\": {
      \"keeper\": \"$keeper_addr\"
    }
  }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES" \
    -y -o json | jq
}

# ─── Remove Keeper Function ──────────────────────────────────────────────────
remove_keeper() {
  if [[ $# -lt 1 ]]; then
    echo "Usage: $0 remove_keeper <keeper_address>"
    echo "Example: $0 remove_keeper zig1your_wallet_address_here"
    exit 1
  fi
  local keeper_addr=$1

  echo "➖ Removing keeper: $keeper_addr"
  echo ""

  $BINARY tx wasm execute "$MAKER_CONTRACT" "{
    \"remove_keeper\": {
      \"keeper\": \"$keeper_addr\"
    }
  }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES" \
    -y -o json | jq
}

# ─── Test Collect Function ──────────────────────────────────────────────────
test_collect() {
  echo "🧪 Testing collect function..."
  echo ""

  $BINARY tx wasm execute "$MAKER_CONTRACT" "{
    \"collect\": {
      \"assets\": []
    }
  }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES" \
    -y -o json | jq
}

# ─── Query Keepers Function ──────────────────────────────────────────────────
query_keepers() {
  echo "🔍 Querying current keepers..."
  echo ""

  $BINARY query wasm contract-state smart "$MAKER_CONTRACT" '{"config":{}}' \
    --node "$RPC_URL" \
    --chain-id "$CHAIN_ID" \
    -o json | jq '.data.authorized_keepers'
}

# ─── Main Function ───────────────────────────────────────────────────────────
main() {
  case "${1:-}" in
    "add_keeper")
      add_keeper "${@:2}"
      ;;
    "remove_keeper")
      remove_keeper "${@:2}"
      ;;
    "test_collect")
      test_collect
      ;;
    "query_keepers")
      query_keepers
      ;;
    *)
      echo "🧪 Keeper Management Test Script"
      echo ""
      echo "Usage: $0 <command> [args]"
      echo ""
      echo "Commands:"
      echo "  add_keeper <address>     - Add a keeper to the maker contract"
      echo "  remove_keeper <address>  - Remove a keeper from the maker contract"
      echo "  test_collect             - Test collect function"
      echo "  query_keepers            - Query current authorized keepers"
      echo ""
      echo "Examples:"
      echo "  $0 add_keeper zig1your_wallet_address_here"
      echo "  $0 remove_keeper zig1your_wallet_address_here"
      echo "  $0 test_collect"
      echo "  $0 query_keepers"
      echo ""
      echo "Note: Make sure to use the correct wallet address for testing!"
      ;;
  esac
}

# ─── Run Main Function ───────────────────────────────────────────────────────
main "$@" 