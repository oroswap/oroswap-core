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
    echo "❌ STAKING_CONTRACT is not set in devnet_testing.env."
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

self_addr() {
    $BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND"
}

# ─── Staking Operations ─────────────────────────────────────────────────────
#
# Architecture note: this is a 1:1 ORO stake registry. STAKES[user] tracks
# the exact ORO amount staked — there are no shares, no exchange rate, no
# bootstrap principal. Use `query_stake_of` and `query_total_shares` to
# inspect the per-user and total stake balances.

# Usage: enter <amount>
# ORO arrives via info.funds; the contract credits STAKES[info.sender] += amount
# at 1:1. There is no on-chain receiver indirection.
enter() {
    local amount="$1"
    echo "📥 Staking $amount $ORO..."
    execute '{"enter":{}}' "--amount $amount$ORO"
}

# Usage: leave <amount> [recipient]
# `amount` is a typed Uint128 of ORO stake to unbond (1:1) — NOT a bank-funds amount.
# `recipient` defaults to the caller; it is the address that will receive the
# ORO when claim() is invoked after the unbonding window.
leave() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: leave <amount> [recipient]"; exit 1; fi
    local amount="$1"
    local recipient="${2:-$(self_addr)}"

    echo "📤 Initiating Unbonding: $amount ORO → $recipient..."
    execute "{\"leave\":{\"amount\":\"$amount\",\"recipient\":\"$recipient\"}}" ""
}

# Usage: claim
claim() {
    echo "💰 Claiming matured unbonding entries..."
    execute '{"claim":{}}' ""
}

# Usage: add_reward <amount> <denom> <duration_seconds>
# Denom and rate are derived from info.funds and duration:
#   denom = first coin in --amount
#   rps   = amount / duration
add_reward() {
    if [[ $# -lt 3 ]]; then echo "❌ Usage: add_reward <amount> <denom> <duration>"; exit 1; fi
    local amount="$1"
    local denom="$2"
    local duration="$3"

    echo "🎁 Adding Reward: $amount $denom over ${duration}s (rps derived = $((amount / duration)))..."
    execute "{\"add_reward\":{\"duration\":$duration}}" \
        "--amount $amount$denom"
}

# Usage: update_config [period] [minimum]
#   period  — new unbonding_period in seconds, or empty/null to skip.
#   minimum — new minimum_stake_amount (Uint128 as base units), or empty to skip.
# Ownership changes go via propose_new_owner / claim_ownership (two-step flow).
update_config() {
    local period="${1:-null}"
    local minimum="${2:-}"

    local minimum_json="null"
    if [[ -n "$minimum" && "$minimum" != "null" ]]; then
        minimum_json="\"$minimum\""
    fi

    echo "⚙️ Updating Config (Period: $period, Minimum: $minimum_json)..."
    execute "{\"update_config\":{\"unbonding_period\":$period,\"minimum_stake_amount\":$minimum_json}}" ""
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
    execute "{\"add_pause_authorities\":{\"addresses\":[$json_addrs]}}" ""
}

# Usage: remove_pause_authorities <addr1> [addr2] ...
remove_pause_authorities() {
    if [[ $# -lt 1 ]]; then echo "❌ Usage: remove_pause_authorities <addr1> [addr2] ..."; exit 1; fi
    local json_addrs
    json_addrs=$(printf '"%s",' "$@" | sed 's/,$//')
    echo "➖ Removing pause authorities: $*"
    execute "{\"remove_pause_authorities\":{\"addresses\":[$json_addrs]}}" ""
}

# ─── Excess-token recovery (audit N-1) ──────────────────────────────────────
#
# ORO is NON-RECOVERABLE: it is the principal asset of the stake registry.
# Every wei the contract holds must remain available to honour Enter/Leave/
# Claim at 1:1. Direct bank-sends of ORO are NOT credited to anyone's stake
# and cannot be swept either — they sit in the contract balance as inert
# excess. Calling withdraw_excess_tokens with the ORO denom returns the
# typed `OroNonRecoverable` error. Reward denoms are likewise guarded.
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

# Sweep orphaned reward tokens (emissions that elapsed while TOTAL_STAKES was
# zero) to `recipient`, or to the contract owner when omitted. Owner only.
#
# Usage: claim_orphaned_rewards [recipient]
claim_orphaned_rewards() {
    local recipient="${1:-null}"
    local recipient_json="null"
    [[ "$recipient" != "null" ]] && recipient_json="\"$recipient\""
    echo "🧹 Sweeping orphaned rewards (recipient=$recipient)..."
    execute "{\"claim_orphaned_rewards\":{\"recipient\":$recipient_json}}" ""
}

# ─── Queries ────────────────────────────────────────────────────────────────

query_config() {
    echo "🔍 Querying Config..." >&2
    query '{"config":{}}'
}

query_state() {
    echo "🔍 Querying Pool State (total ORO staked)..." >&2
    query '{"state":{}}'
}

# Per-user ORO stake balance from STAKES.
query_stake_of() {
    local addr="${1:-$(self_addr)}"
    echo "🔍 Querying Stake (ORO) for $addr..." >&2
    query "{\"stake_of\":{\"address\":\"$addr\"}}"
}

# Global stake total from TOTAL_STAKES.
query_total_shares() {
    echo "📊 Querying Total Staked ORO..." >&2
    query '{"total_shares":{}}'
}

query_total_locked() {
    echo "💰 Querying Total ORO Locked (Unbonding)..." >&2
    query '{"total_locked":{}}'
}

query_unbonding() {
    local addr="${1:-$(self_addr)}"
    echo "📝 Querying Unbonding Entries for $addr..." >&2
    query "{\"unbonding_entries\":{\"address\":\"$addr\"}}"
}

query_pending_rewards() {
    local addr="${1:-$(self_addr)}"
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
    local addr="${1:-$(self_addr)}"
    echo "📋 Staking Status for $addr:"
    query_config
    query_state
    query_stake_of "$addr"
    query_pending_rewards "$addr"
    query_unbonding "$addr"
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  enter <amount>                                            # Stake ORO 1:1 (credits sender's STAKES entry)"
  echo "  leave <amount> [recipient]                                # Debit stake → queue unbonding entry"
  echo "  claim                                                     # Claim matured ORO + side-stream rewards"
  echo "  add_reward <amt> <denom> <sec>                            # (Admin) Add reward stream (rps derived)"
  echo "  withdraw_excess_tokens <denom> [recipient]                # (Admin) Sweep non-ORO/non-reward tokens"
  echo "  update_config [period] [minimum]                          # (Admin) Update unbonding period and/or minimum stake amount"
  echo "  pause                                                     # Owner OR pause-authority"
  echo "  unpause                                                   # Owner ONLY"
  echo "  add_pause_authorities <addr1> [addr2] ...                 # (Admin) Grant pause authority"
  echo "  remove_pause_authorities <addr1> [addr2] ...              # (Admin) Revoke pause authority"
  echo "  propose_new_owner <addr> <expires_in>                     # Two-step owner transfer"
  echo "  drop_ownership_proposal                                   # Drop pending proposal"
  echo "  claim_ownership                                           # Claim proposed ownership"
  echo "  claim_orphaned_rewards [recipient]                        # (Admin) Sweep orphan-bucket emissions (no-staker windows)"
  echo "  query_config                                              # Get contract config"
  echo "  query_state                                               # Pool: total staked ORO"
  echo "  query_stake_of [address]                                  # Per-user ORO stake (STAKES map)"
  echo "  query_total_shares                                        # Global total staked ORO (TOTAL_STAKES)"
  echo "  query_total_locked                                        # Total ORO in unbonding"
  echo "  query_unbonding [address]                                 # Get unbonding entries"
  echo "  query_unbonding_period                                    # Global unbonding seconds"
  echo "  query_pending_rewards [address]                           # Pending side-stream rewards"
  echo "  query_pause_authorities                                   # Extra pause-authorised addrs"
  echo "  status [address]                                          # Show all relevant info for user"
  exit 0
fi

case "$1" in
  enter)                    shift; enter "$@" ;;
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
  claim_orphaned_rewards)   shift; claim_orphaned_rewards "$@" ;;
  query_config)             query_config ;;
  query_state)              query_state ;;
  query_stake_of)           shift; query_stake_of "$@" ;;
  query_total_shares)       query_total_shares ;;
  query_total_locked)       query_total_locked ;;
  query_unbonding)          shift; query_unbonding "$@" ;;
  query_unbonding_period)   query_unbonding_period ;;
  query_pending_rewards)    shift; query_pending_rewards "$@" ;;
  query_pause_authorities)  query_pause_authorities ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
