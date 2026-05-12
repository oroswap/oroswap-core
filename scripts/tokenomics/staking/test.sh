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

# Usage: enter_with_hook <amount> <contract_address> <base64_msg>
# Forwards the resulting xORO (in full) to <contract_address> along with the
# supplied executable msg in a single atomic call. Useful for composing into
# other protocols that accept xORO.
enter_with_hook() {
    if [[ $# -lt 3 ]]; then
        echo "❌ Usage: enter_with_hook <amount> <contract_address> <base64_msg>"
        exit 1
    fi
    local amount="$1"
    local contract="$2"
    local msg_b64="$3"
    echo "📥➡️  Staking $amount $ORO and forwarding xORO to $contract..."
    execute "{\"enter_with_hook\":{\"contract_address\":\"$contract\",\"msg\":\"$msg_b64\"}}" \
        "--amount $amount$ORO"
}

# ─── Pause / Pause-authority ────────────────────────────────────────────────
#
# Asymmetric authority pattern (audit item 5): any address in
# PAUSE_AUTHORITIES can `pause`, but ONLY the owner can `unpause`. Limits
# blast radius of a compromised authority key — they can grief by pausing
# but cannot toggle pause state back and forth.

pause() {
    echo "⏸  Pausing contract (owner or any pause-authority)..."
    execute '{"pause":{}}' ""
}

unpause() {
    echo "▶️  Unpausing contract (owner only)..."
    execute '{"unpause":{}}' ""
}

# Usage: add_pause_authorities <addr1> [addr2] ...
add_pause_authorities() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: add_pause_authorities <addr1> [addr2] ..."; exit 1; fi
    local json_addrs
    json_addrs=$(printf '"%s",' "$@" | sed 's/,$//')
    echo "➕ Adding pause authorities: $*"
    execute "{\"add_pause_authorities\":{\"authorities\":[$json_addrs]}}" ""
}

# Usage: remove_pause_authorities <addr1> [addr2] ...
remove_pause_authorities() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: remove_pause_authorities <addr1> [addr2] ..."; exit 1; fi
    local json_addrs
    json_addrs=$(printf '"%s",' "$@" | sed 's/,$//')
    echo "➖ Removing pause authorities: $*"
    execute "{\"remove_pause_authorities\":{\"authorities\":[$json_addrs]}}" ""
}

# ─── Excess-token recovery (audit N-1) ──────────────────────────────────────
#
# ORO is NON-RECOVERABLE: every wei held by the contract is owed to stakers,
# unbonders, or the reward stream. Direct bank-sends of ORO auto-compound by
# design. Calling withdraw_excess_tokens with the ORO denom returns the
# typed `OroNonRecoverable` error.
#
# Usage: withdraw_excess_tokens <denom> [recipient]
withdraw_excess_tokens() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: withdraw_excess_tokens <denom> [recipient]"; exit 1; fi
    local denom="$1"
    local recipient="${2:-null}"
    local recipient_json="null"
    [[ "$recipient" != "null" ]] && recipient_json="\"$recipient\""
    echo "🧹 Sweeping excess $denom..."
    execute "{\"withdraw_excess_tokens\":{\"denom\":\"$denom\",\"recipient\":$recipient_json}}" ""
}

# ─── Ownership transfer (two-step) ──────────────────────────────────────────

# Usage: propose_new_owner <new_owner> <expires_in_seconds>
propose_new_owner() {
    if [[ $# -lt 2 ]]; then echo "❌ Usage: propose_new_owner <new_owner> <expires_in>"; exit 1; fi
    local new_owner="$1"
    local expires_in="$2"
    echo "⚙️  Proposing new owner $new_owner (expires in ${expires_in}s)..."
    execute "{\"propose_new_owner\":{\"owner\":\"$new_owner\",\"expires_in\":$expires_in}}" ""
}

drop_ownership_proposal() {
    echo "⚙️  Dropping pending ownership proposal..."
    execute '{"drop_ownership_proposal":{}}' ""
}

claim_ownership() {
    echo "⚙️  Claiming ownership..."
    execute '{"claim_ownership":{}}' ""
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

query_pause_authorities() {
    echo "🔍 Querying Pause Authorities..." >&2
    query '{"pause_authorities":{}}'
}

query_unbonding_period() {
    echo "⏳ Querying Unbonding Period..." >&2
    query '{"unbonding_period":{}}'
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
  echo "  enter <amount> [receiver]                                 # Stake ORO"
  echo "  enter_with_hook <amount> <contract> <base64_msg>          # Stake + forward xORO with hook"
  echo "  leave <amount_xoro> [receiver]                            # Start unbonding"
  echo "  claim                                                     # Claim matured ORO + rewards"
  echo "  add_reward <amt> <denom> <sec>                            # (Admin) Add reward stream"
  echo "  withdraw_excess_tokens <denom> [recipient]                # (Admin) Sweep non-ORO tokens"
  echo "  update_config [period]                                    # (Admin) Update unbonding period"
  echo "  pause                                                     # Owner OR pause-authority"
  echo "  unpause                                                   # Owner ONLY"
  echo "  add_pause_authorities <addr1> [addr2] ...                 # (Admin) Grant pause authority"
  echo "  remove_pause_authorities <addr1> [addr2] ...              # (Admin) Revoke pause authority"
  echo "  propose_new_owner <addr> <expires_in>                     # Two-step owner transfer"
  echo "  drop_ownership_proposal                                   # Drop pending proposal"
  echo "  claim_ownership                                           # Claim proposed ownership"
  echo "  query_config                                              # Get contract config"
  echo "  query_total_deposit                                       # Total ORO in pool (active)"
  echo "  query_state                                               # Total ORO, shares, exchange rate"
  echo "  query_total_shares                                        # Total xORO supply"
  echo "  query_total_locked                                        # Total ORO in unbonding"
  echo "  query_unbonding [address]                                 # Get unbonding entries"
  echo "  query_unbonding_period                                    # Global unbonding seconds"
  echo "  query_balance_at <address>                                # Current xORO balance"
  echo "  query_pending_rewards [address]                           # Pending side-stream rewards"
  echo "  query_pause_authorities                                   # Extra pause-authorised addrs"
  echo "  status [address]                                          # Show all relevant info for user"
  exit 0
fi

case "$1" in
  enter)                    shift; enter "$@" ;;
  enter_with_hook)          shift; enter_with_hook "$@" ;;
  leave)                    shift; leave "$@" ;;
  claim)                    shift; claim "$@" ;;
  add_reward)               shift; add_reward "$@" ;;
  withdraw_excess_tokens)   shift; withdraw_excess_tokens "$@" ;;
  status)                   shift; status "$@" ;;
  update_config)            shift; update_config "$@" ;;
  pause)                    pause ;;
  unpause)                  unpause ;;
  add_pause_authorities)    shift; add_pause_authorities "$@" ;;
  remove_pause_authorities) shift; remove_pause_authorities "$@" ;;
  propose_new_owner)        shift; propose_new_owner "$@" ;;
  drop_ownership_proposal)  drop_ownership_proposal ;;
  claim_ownership)          claim_ownership ;;
  query_config)             query_config ;;
  query_total_deposit)      query_total_deposit ;;
  query_state)              query_state ;;
  query_total_shares)       query_total_shares ;;
  query_total_locked)       query_total_locked ;;
  query_unbonding)          shift; query_unbonding "$@" ;;
  query_unbonding_period)   query_unbonding_period ;;
  query_balance_at)         shift; query_balance_at "$@" ;;
  query_pending_rewards)    shift; query_pending_rewards "$@" ;;
  query_pause_authorities)  query_pause_authorities ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
