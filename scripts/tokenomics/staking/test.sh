#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
# Source environment variables if not already set
if [[ -z "${BINARY:-}" ]]; then
    source "$(dirname "$0")/../../devnet_testing.env"
fi

STAKING="${STAKING_CONTRACT:-}"
ORO="${ORO_TOKEN:-}"

if [[ -z "$STAKING" ]]; then
    echo "❌ STAKING_CONTRACT is not set in devtnet_testing.env."
    exit 1
fi

# ─── Helper Functions ────────────────────────────────────────────────────────
query() {
    $BINARY query wasm contract-state smart "$STAKING" "$1" --node "$RPC_URL" -o json | jq
}

execute() {
    local msg="$1"
    local extra_args="${2:-}"
    $BINARY tx wasm execute "$STAKING" "$msg" --from "$KEY_NAME" \
      --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" --chain-id "$CHAIN_ID" \
      --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" $extra_args -y -o json | jq
}

# ─── Staking Operations ─────────────────────────────────────────────────────

# Usage: enter <amount> [receiver]
enter() {
    local amount="$1"
    local receiver="${2:-null}"
    local receiver_json="null"
    [[ "$receiver" != "null" ]] && receiver_json="\"$receiver\""

    echo "📥 Staking $amount $ORO..."
    execute "{\"enter\":{\"receiver\":$receiver_json}}" "--amount $amount$ORO"
}

# Usage: leave <amount_xoro> [receiver]
leave() {
    local amount="$1"
    local receiver="${2:-null}"
    local receiver_json="null"
    [[ "$receiver" != "null" ]] && receiver_json="\"$receiver\""

    echo "📤 Initiating Unbonding: $amount xORO..."
    execute "{\"leave\":{\"receiver\":$receiver_json}}" "--amount $amount$(query_config | jq -r '.data.xoro_denom')"
}

# Usage: claim
claim() {
    echo "💰 Claiming matured unbonding entries..."
    execute "{\"claim\":{}}" ""
}

# Usage: add_reward <amount> <denom> <duration_seconds>
add_reward() {
    local amount="$1"
    local denom="$2"
    local duration="$3"

    echo "🎁 Adding Reward: $amount $denom for $duration seconds..."
    execute "{\"add_reward\":{\"duration\":$duration}}" "--amount $amount$denom"
}

# Usage: update_config [new_unbonding_period]
update_config() {
    local period="${1:-null}"

    echo "⚙️ Updating Config (Period: $period)..."
    execute "{\"update_config\":{\"unbonding_period\":$period}}" ""
}


# ─── Queries ────────────────────────────────────────────────────────────────

query_config() {
    echo "🔍 Querying Config..." >&2
    query '{"config":{}}'
}

query_total_deposit() {
    echo "🔍 Querying Total Deposit (Effective Pool)..." >&2
    query '{"total_deposit":{}}'
}

query_state() {
    echo "🔍 Querying Pool State (Exchange Rate)..." >&2
    query '{"state":{}}'
}

query_total_shares() {
    echo "📊 Querying Total Shares..." >&2
    query '{"total_shares":{}}'
}

query_total_locked() {
    echo "💰 Querying Total ORO Locked (Unbonding)..." >&2
    query '{"total_locked":{}}'
}

query_unbonding() {
    local addr="${1:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    echo "📝 Querying Unbonding Entries for $addr..." >&2
    query "{\"unbonding_entries\":{\"address\":\"$addr\"}}"
}

query_balance_at() {
    local addr="$1"
    echo "🔍 Querying current xORO Balance for $addr..." >&2
    query "{\"balance_at\":{\"address\":\"$addr\",\"timestamp\":null}}"
}

query_pending_rewards() {
    local addr="${1:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    echo "🎁 Querying Pending Rewards for $addr..." >&2
    query "{\"pending_rewards\":{\"address\":\"$addr\"}}"
}

status() {
    local addr="${1:-$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")}"
    echo "📋 Staking Status for $addr:"
    query_config
    query_state
    query_pending_rewards "$addr"
    query_unbonding "$addr"
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  enter <amount> [receiver]          # Stake ORO"
  echo "  leave <amount_xoro> [receiver]     # Start unbonding"
  echo "  claim                              # Claim matured ORO"
  echo "  update_config [period]             # Update unbonding period"
  echo "  query_config                       # Get contract config"
  echo "  query_total_deposit                # Get total ORO in pool (active)"
  echo "  query_state                        # Get total ORO, shares and price"
  echo "  query_total_shares                 # Get total xORO supply"
  echo "  query_total_locked                 # Get total ORO in unbonding"
  echo "  query_unbonding [address]          # Get unbonding entries"
  echo "  query_balance_at <address>         # Get current xORO balance"
  echo "  query_pending_rewards [address]    # Get pending rewards"
  echo "  add_reward <amt> <denom> <sec>     # (Admin) Add incentive rewards"
  echo "  status [address]                   # Show all relevant info for user"
  exit 0
fi

case "$1" in
  enter)              shift; enter "$@" ;;
  leave)              shift; leave "$@" ;;
  claim)              shift; claim "$@" ;;
  add_reward)         shift; add_reward "$@" ;;
  status)             shift; status "$@" ;;
  update_config)      shift; update_config "$@" ;;
  query_config)       query_config ;;
  query_total_deposit) query_total_deposit ;;
  query_state)        query_state ;;
  query_total_shares) query_total_shares ;;
  query_total_locked) query_total_locked ;;
  query_unbonding)    shift; query_unbonding "$@" ;;
  query_balance_at)   shift; query_balance_at "$@" ;;
  query_pending_rewards) shift; query_pending_rewards "$@" ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
