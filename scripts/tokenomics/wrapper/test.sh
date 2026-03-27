#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
source "$(dirname "$0")/../../testnet_testing.env"

WRAPPER="${WRAPPER_CONTRACT:-}"

if [[ -z "$WRAPPER" ]]; then
    echo "❌ WRAPPER_CONTRACT is not set. Please set it in your environment or testnet_testing.env."
    # exit 1
fi

# ─── Helper Functions ────────────────────────────────────────────────────────
query() {
    $BINARY query wasm contract-state smart "$WRAPPER" "$1" --node "$RPC_URL" -o json | jq
}

execute() {
    local msg="$1"
    local extra_args="${2:-}"
    $BINARY tx wasm execute "$WRAPPER" "$msg" --from "$KEY_NAME" \
      --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" --chain-id "$CHAIN_ID" \
      --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" $extra_args -y -o json | jq
}

# ─── Wrapper Operations ─────────────────────────────────────────────────────

# Usage: lock <amount> <lock_denom>
lock() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: lock <amount> <lock_denom>"; exit 1; fi
    local amount="$1"
    local denom="$2"
    echo "🔒 Locking $amount $denom..."
    execute "{\"lock\":{}}" "--amount $amount$denom"
}

# Usage: unlock <amount> <mint_denom>
unlock() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: unlock <amount> <mint_denom>"; exit 1; fi
    local amount="$1"
    local denom="$2"
    echo "🔓 Unlocking $amount $denom..."
    execute "{\"unlock\":{}}" "--amount $amount$denom"
}

pause() {
    echo "⏸ Pausing contract..."
    execute "{\"pause\":{}}" ""
}

unpause() {
    echo "▶️ Unpausing contract..."
    execute "{\"unpause\":{}}" ""
}

# ─── Queries ────────────────────────────────────────────────────────────────

query_config() {
    echo "🔍 Querying Config..." >&2
    query '{"config":{}}'
}

query_total_locked() {
    echo "📊 Querying Total Locked..." >&2
    query '{"total_locked":{}}'
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  lock <amount> <denom>        # Lock tokens"
  echo "  unlock <amount> <denom>      # Unlock tokens"
  echo "  pause                        # Pause operations"
  echo "  unpause                      # Unpause operations"
  echo "  query_config                 # Get contract config"
  echo "  query_total_locked           # Get total locked amount"
  exit 0
fi

case "$1" in
  lock)               shift; lock "$@" ;;
  unlock)             shift; unlock "$@" ;;
  pause)              pause ;;
  unpause)            unpause ;;
  query_config)       query_config ;;
  query_total_locked) query_total_locked ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
