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

# ─── Metadata management ────────────────────────────────────────────────────
#
# `UpdateDenomMetadata` has TRUE PATCH semantics: each `null` argument keeps
# the value currently stored on-chain; only non-null fields are overwritten.
# Pass an empty string ("") for any field you want to leave alone — the
# helper translates it to `null` before sending.
#
# Decimals are NOT updatable. The exponent was baked in at instantiate via
# `mint_decimals` and the contract reads `config.mint_decimals` on every
# patch — no need to (and no way to) supply it here.
#
# Usage: update_denom_metadata <name|""> <symbol|""> <description|""> <uri|"">
update_denom_metadata() {
    if [[ $# -lt 4 ]]; then
        echo "❌ Usage: update_denom_metadata <name|\"\"> <symbol|\"\"> <description|\"\"> <uri|\"\">"
        echo "   Pass \"\" for any field you want to leave unchanged (patch semantics)."
        exit 1
    fi
    local name_arg="$1"
    local symbol_arg="$2"
    local desc_arg="$3"
    local uri_arg="$4"

    fmt() {
        if [[ -z "$1" ]]; then echo "null"; else echo "\"$1\""; fi
    }

    local name_json symbol_json desc_json uri_json
    name_json=$(fmt "$name_arg")
    symbol_json=$(fmt "$symbol_arg")
    desc_json=$(fmt "$desc_arg")
    uri_json=$(fmt "$uri_arg")

    echo "🏷  Patching denom metadata (only non-null fields will change)..."
    execute "{\"update_denom_metadata\":{\"name\":$name_json,\"symbol\":$symbol_json,\"description\":$desc_json,\"uri\":$uri_json}}" ""
}

# Usage: propose_denom_admin <bank_admin> <metadata_admin>
# ⚠️ HIGH BLAST RADIUS — run AFTER MintSupply has drained to 0.
# The wrapper loses mint/burn rights once the new admins claim. Any
# remaining xORO holders become permanently unredeemable through this
# contract. Operational order:
#   1. pause             — stop new locks
#   2. wait for users to unlock until query_mint_supply == 0
#   3. propose_denom_admin <new_bank_admin> <new_metadata_admin>
#   4. new admin runs `zigchaind tx factory claim-denom-admin <denom>`
propose_denom_admin() {
    if [[ $# -lt 2 ]]; then
        echo "❌ Usage: propose_denom_admin <bank_admin> <metadata_admin>"
        echo "   ⚠️  This hands off mint/burn rights. Drain MintSupply to 0 first."
        exit 1
    fi
    local bank_admin="$1"
    local metadata_admin="$2"
    local supply
    supply=$(query '{"mint_supply":{}}' 2>/dev/null | jq -r '.data')
    if [[ -n "$supply" && "$supply" != "0" && "$supply" != "null" ]]; then
        echo "⚠️  Current MintSupply = $supply (NOT zero)."
        echo "    Handing off the denom admin now will brick existing holders."
        read -p "    Continue anyway? [y/N] " confirm
        [[ "$confirm" == "y" || "$confirm" == "Y" ]] || { echo "Aborted."; exit 1; }
    fi
    echo "🔑 Proposing new denom admins (bank=$bank_admin, metadata=$metadata_admin)..."
    execute "{\"propose_denom_admin\":{\"bank_admin\":\"$bank_admin\",\"metadata_admin\":\"$metadata_admin\"}}" ""
}

# ─── Pause-authority management ─────────────────────────────────────────────

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

# ─── Maintenance ────────────────────────────────────────────────────────────

recover_excess() {
    echo "🧹 Recovering excess (only the portion of lock_denom held without a matching mint)..."
    execute '{"recover_excess":{}}' ""
}

# ─── Ownership transfer (two-step) ──────────────────────────────────────────

# Usage: propose_new_owner <new_owner_address> <expires_in_seconds>
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

query_total_locked() {
    echo "📊 Querying Total Locked..." >&2
    query '{"total_locked":{}}'
}

query_mint_supply() {
    echo "📊 Querying Mint Supply..." >&2
    query '{"mint_supply":{}}'
}

query_pause_authorities() {
    echo "🔍 Querying Pause Authorities..." >&2
    query '{"pause_authorities":{}}'
}

# Print a side-by-side view of TotalLocked vs MintSupply — the 1:1 invariant
# holds iff they are equal (otherwise the diff is recoverable via
# `recover_excess`).
query_invariant() {
    local locked supply
    locked=$(query '{"total_locked":{}}' 2>/dev/null | jq -r '.data')
    supply=$(query '{"mint_supply":{}}' 2>/dev/null | jq -r '.data')
    echo "📐 Invariant check (should be equal):"
    echo "    total_locked = $locked"
    echo "    mint_supply  = $supply"
    if [[ "$locked" == "$supply" ]]; then
        echo "    ✅ 1:1 backed"
    else
        echo "    ⚠️  divergence detected — diff is recoverable via recover_excess"
    fi
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  lock <amount> <denom>                                       # Lock tokens"
  echo "  unlock <amount> <denom>                                     # Unlock tokens"
  echo "  pause                                                       # Pause (owner or pause-authority)"
  echo "  unpause                                                     # Unpause (owner only)"
  echo "  update_denom_metadata <name> <symbol> <description> <uri>   # Patch metadata; pass \"\" to keep field unchanged"
  echo "  propose_denom_admin <bank_admin> <metadata_admin>           # Retire: drain MintSupply to 0 first ⚠️"
  echo "  add_pause_authorities <addr1> [addr2] ...                   # Grant pause authority"
  echo "  remove_pause_authorities <addr1> [addr2] ...                # Revoke pause authority"
  echo "  recover_excess                                              # Restore 1:1 invariant"
  echo "  propose_new_owner <addr> <expires_in>                       # Two-step owner transfer"
  echo "  drop_ownership_proposal                                     # Drop pending proposal"
  echo "  claim_ownership                                             # Claim proposed ownership"
  echo "  query_config                                                # Get contract config"
  echo "  query_total_locked                                          # lock_denom balance"
  echo "  query_mint_supply                                           # outstanding mint_denom"
  echo "  query_pause_authorities                                     # extra pause-authorised addrs"
  echo "  query_invariant                                             # total_locked vs mint_supply"
  exit 0
fi

case "$1" in
  lock)                       shift; lock "$@" ;;
  unlock)                     shift; unlock "$@" ;;
  pause)                      pause ;;
  unpause)                    unpause ;;
  update_denom_metadata)      shift; update_denom_metadata "$@" ;;
  propose_denom_admin)        shift; propose_denom_admin "$@" ;;
  add_pause_authorities)      shift; add_pause_authorities "$@" ;;
  remove_pause_authorities)   shift; remove_pause_authorities "$@" ;;
  recover_excess)             recover_excess ;;
  propose_new_owner)          shift; propose_new_owner "$@" ;;
  drop_ownership_proposal)    drop_ownership_proposal ;;
  claim_ownership)            claim_ownership ;;
  query_config)               query_config ;;
  query_total_locked)         query_total_locked ;;
  query_mint_supply)          query_mint_supply ;;
  query_pause_authorities)    query_pause_authorities ;;
  query_invariant)            query_invariant ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
