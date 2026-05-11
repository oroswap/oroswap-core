#!/bin/bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
# Source environment variables
source "$(dirname "$0")/../../devnet_testing.env"

AIRDROP="${AIRDROP_CONTRACT:-}"

if [[ -z "$AIRDROP" ]]; then
    echo "❌ AIRDROP_CONTRACT is not set. Please set it in your environment or devnet_testing.env."
    # exit 1
fi

# ─── Helper Functions ────────────────────────────────────────────────────────
query() {
    $BINARY query wasm contract-state smart "$AIRDROP" "$1" --node "$RPC_URL" -o json | jq
}

execute() {
    local msg="$1"
    local extra_args="${2:-}"
    $BINARY tx wasm execute "$AIRDROP" "$msg" --from "$KEY_NAME" \
      --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" --chain-id "$CHAIN_ID" \
      --gas-prices "$GAS_PRICES" --gas auto --gas-adjustment "$GAS_ADJUSTMENT" $extra_args -y -o json | jq
}

# ─── Airdrop Operations ─────────────────────────────────────────────────────

# Usage: add_airdrop <token_address> <start_time> <v_start> <v_end> <initial_percent> <apy_bps> <eligibility_json> [expiry]
# Contract limits:
#   initial_claim_percent <= 100
#   apy_bonus_bps <= 10_000 (100%)
#   eligibility JSON: <= 200 entries per call (use update_eligibility for more)
#   start_time >= now; vesting_start_time in [start_time, vesting_end_time)
#   expiry_time (if set) > vesting_end_time
add_airdrop() {
    if [[ $# -lt 7 ]]; then echo "❌ Usage: add_airdrop <token> <start> <v_start> <v_end> <initial_claim_percent> <apy_bps> <eligibility_json> [expiry]"; exit 1; fi
    local token="$1"
    local start="$2"
    local v_start="$3"
    local v_end="$4"
    local initial_percent="$5"
    local apy="$6"
    local eligibility="$7"
    local expiry="${8:-null}"

    echo "🆕 Adding new airdrop for $token..."
    local is_native="false"
    if [[ "$token" == *"."* || ${#token} -lt 20 ]]; then
        is_native="true"
    fi
    execute "{\"add_airdrop\":{\"token_address\":\"$token\",\"is_native\":$is_native,\"start_time\":$start,\"vesting_start_time\":$v_start,\"vesting_end_time\":$v_end,\"initial_claim_percent\":$initial_percent,\"apy_bonus_bps\":$apy,\"eligibility\":$eligibility,\"expiry_time\":$expiry}}" ""
}

# Usage: claim <airdrop_id>
claim() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: claim <airdrop_id>"; exit 1; fi
    local id="$1"
    echo "💰 Claiming airdrop $id..."
    execute "{\"claim\":{\"airdrop_id\":$id}}" ""
}

# Usage: toggle_airdrop <id> <active>
toggle_airdrop() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: toggle_airdrop <id> <true/false>"; exit 1; fi
    local id="$1"
    local active="$2"
    echo "🔄 Toggling airdrop $id to active=$active..."
    execute "{\"toggle_airdrop\":{\"id\":$id,\"active\":$active}}" ""
}

# Usage: deposit_funds <token_address> <amount>
deposit_funds() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: deposit_funds <token_address> <amount>"; exit 1; fi
    local token="$1"
    local amount="$2"
    echo "📥 Depositing $amount tokens ($token) to airdrop contract..."
    
    local extra=""
    local is_native="false"
    if [[ "$token" == *"."* || ${#token} -lt 20 || "$token" == "ibc/"* ]]; then
        is_native="true"
        extra="--amount $amount$token"
    fi
    
    execute "{\"deposit_funds\":{\"token_address\":\"$token\",\"amount\":\"$amount\",\"is_native\":$is_native}}" "$extra"
}

# Usage: withdraw_funds <token_address> <amount>
# NOTE: blocked while ANY airdrop using this token is still active OR has
# no expiry_time configured OR has not yet reached its expiry. To unblock
# you must:
#   1. test.sh toggle_airdrop <id> false
#   2. wait until block time >= the airdrop's expiry_time
withdraw_funds() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: withdraw_funds <token_address> <amount>"; exit 1; fi
    local token="$1"
    local amount="$2"
    echo "📤 Withdrawing $amount tokens ($token) from airdrop contract..."
    echo "ℹ️  If this fails with WithdrawalBlocked: deactivate the airdrop and wait for expiry_time."

    local is_native="false"
    if [[ "$token" == *"."* || ${#token} -lt 20 || "$token" == "ibc/"* ]]; then
        is_native="true"
    fi
    execute "{\"withdraw_funds\":{\"token_address\":\"$token\",\"amount\":\"$amount\",\"is_native\":$is_native}}" ""
}

# ─── Queries ────────────────────────────────────────────────────────────────

query_config() {
    echo "🔍 Querying Config..." >&2
    query '{"config":{}}'
}

query_airdrop() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: query_airdrop <id>"; exit 1; fi
    local id="$1"
    echo "🔍 Querying Airdrop $id..." >&2
    query "{\"airdrop\":{\"id\":$id}}"
}

query_user_airdrop_status() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: query_user_airdrop_status <id> <address>"; exit 1; fi
    local id="$1"
    local addr="$2"
    echo "🔍 Querying User Airdrop Status for $addr in airdrop $id..." >&2
    query "{\"user_airdrop_status\":{\"id\":$id,\"address\":\"$addr\"}}"
}

query_latest_id() {
    echo "📊 Querying Latest Airdrop ID..." >&2
    query '{"latest_airdrop_id":{}}'
}

# Solvency snapshot for an airdrop:
#   committed eligibility, total claimed so far, contract balance, and
#   whether the pool is fully funded for the worst-case payout.
query_solvency() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: query_solvency <id>"; exit 1; fi
    local id="$1"
    echo "🧮 Querying Solvency for airdrop $id..." >&2
    query "{\"solvency\":{\"id\":$id}}"
}

# Combined status check
status() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: status <id> <address>"; exit 1; fi
    local id="$1"
    local addr="$2"
    echo "📋 Status for Airdrop $id, User $addr:"
    query_airdrop "$id"
    query_user_airdrop_status "$id" "$addr"
}

propose_new_owner() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: propose_new_owner <new_admin_address> <expires_in_seconds>"; exit 1; fi
    local new_admin="$1"
    local expires="$2"
    echo "⚙️ Proposing new owner $new_admin..."
    execute "{\"propose_new_owner\":{\"owner\":\"$new_admin\",\"expires_in\":$expires}}" ""
}

drop_ownership_proposal() {
    echo "⚙️ Dropping ownership proposal..."
    execute "{\"drop_ownership_proposal\":{}}" ""
}

claim_ownership() {
    echo "⚙️ Claiming ownership..."
    execute "{\"claim_ownership\":{}}" ""
}

update_eligibility() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: update_eligibility <airdrop_id> <eligibility_json>"; exit 1; fi
    local id="$1"
    local eligibility="$2"
    # Soft-warn if the JSON probably has > 200 entries (the contract cap).
    local approx_entries
    approx_entries=$(printf '%s' "$eligibility" | grep -o '"' | wc -l | awk '{print $1 / 4}')
    if (( $(echo "$approx_entries > 200" | bc -l 2>/dev/null || echo 0) )); then
        echo "⚠️  ~${approx_entries} entries detected. Contract limit is 200 per call — split into multiple calls if rejected."
    fi
    echo "⚙️ Updating eligibility for airdrop $id..."
    execute "{\"update_eligibility\":{\"airdrop_id\":$id,\"eligibility\":$eligibility}}" ""
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  claim <id>                                            # Claim tokens (one-time settlement)"
  echo "  toggle_airdrop <id> <active (true/false)>             # Enable/Disable claims"
  echo "  deposit_funds <token> <amount>                        # Deposit tokens"
  echo "  withdraw_funds <token> <amount>                       # Withdraw tokens"
  echo "  add_airdrop <token> <start> <v_start> <v_end> <%> <apy> <json> [expiry] # Add airdrop"
  echo "  update_eligibility <id> <json>                        # Batch add/update eligibility"
  echo "  propose_new_owner <new_admin> <expires_in>            # Propose new admin address"
  echo "  drop_ownership_proposal                               # Drop pending ownership proposal"
  echo "  claim_ownership                                       # Claim ownership"
  echo "  query_config                                          # Get contract config"
  echo "  query_airdrop <id>                                    # Get airdrop info"
  echo "  query_user_airdrop_status <id> <address>              # Check airdrop status and eligibility"
  echo "  query_latest_id                                       # Get latest ID"
  echo "  query_solvency <id>                                   # Eligibility vs claimed vs balance funding check"
  exit 0
fi

case "$1" in
  status)             shift; status "$@" ;;
  add_airdrop)        shift; add_airdrop "$@" ;;
  claim)              shift; claim "$@" ;;
  toggle_airdrop)     shift; toggle_airdrop "$@" ;;
  deposit_funds)      shift; deposit_funds "$@" ;;
  withdraw_funds)     shift; withdraw_funds "$@" ;;
  propose_new_owner)  shift; propose_new_owner "$@" ;;
  drop_ownership_proposal) drop_ownership_proposal ;;
  claim_ownership)    claim_ownership ;;
  update_eligibility) shift; update_eligibility "$@" ;;
  query_config)       query_config ;;
  query_airdrop)      shift; query_airdrop "$@" ;;
  query_user_airdrop_status) shift; query_user_airdrop_status "$@" ;;
  query_latest_id)    query_latest_id ;;
  query_solvency)     shift; query_solvency "$@" ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
