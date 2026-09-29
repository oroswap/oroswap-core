#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# End-to-end test for the UpdateExpiry message on a live chain.
#
# Always provisions a fresh contract instance so prior runs cannot pollute
# state. The optimized wasm at opt-artifacts/oroswap_airdrop.wasm is stored
# and instantiated by this script — make sure it's up to date (run
# scripts/build_single_contract.sh tokenomics/airdrop if you've changed
# contract code).
#
# Exercises the full UpdateExpiry phase gate:
#   T1  pre-start, extend expiry further into future     → success
#   T2  forward-only violation (shrink expiry)           → NonForwardUpdate
#   T3  forward-only violation (equal to current)        → NonForwardUpdate
#   T4  non-admin caller                                 → Unauthorized
#   T5  mid-vesting, extend expiry (allowed)             → success
#
# The "extend after expiry has already passed" branch is not covered here
# because replicating it live would require waiting through the full expiry
# window. It is covered by unit tests.
#
# Requirements:
#   • devnet_testing.env exports KEY_NAME, BINARY, RPC_URL, CHAIN_ID, gas vars
#   • opt-artifacts/oroswap_airdrop.wasm is built
#   • NON_ADMIN_KEY (default: unauthorized_test) — a separate key with gas
#     funds, used to verify the admin gate
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# Capture caller-provided overrides BEFORE sourcing the env file.
_CLI_NON_ADMIN_KEY="${NON_ADMIN_KEY:-}"

source "$(dirname "$0")/../../devnet_testing.env"

NON_ADMIN_KEY="${_CLI_NON_ADMIN_KEY:-${NON_ADMIN_KEY:-unauthorized_test}}"

WASM_PATH="$(cd "$(dirname "$0")/../../.." && pwd)/opt-artifacts/oroswap_airdrop.wasm"

[[ -f "$WASM_PATH" ]] || { echo "❌ wasm not found at $WASM_PATH — build it first"; exit 1; }
$BINARY keys show "$NON_ADMIN_KEY" --keyring-backend "$KEYRING_BACKEND" >/dev/null 2>&1 \
    || { echo "❌ Non-admin key '$NON_ADMIN_KEY' not found in keyring."; exit 1; }

ADMIN=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

PASS=0
FAIL=0
note()  { echo; echo "── $1 ──"; }
pass()  { echo "  ✅ PASS — $1"; PASS=$((PASS+1)); }
fail()  { echo "  ❌ FAIL — $1"; FAIL=$((FAIL+1)); }

# Broadcast a tx, return txhash on success, write any error to stderr.
broadcast() {
    local from="$1"; local msg="$2"; shift 2
    local out
    out=$($BINARY tx wasm execute "$AIRDROP" "$msg" --from "$from" \
        --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" \
        --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
        --gas auto --gas-adjustment "$GAS_ADJUSTMENT" "$@" -y 2>&1)
    local tx
    tx=$(echo "$out" | awk '/^txhash:/{print $2}' | head -1)
    if [[ -z "$tx" ]]; then
        echo "$out" | grep -oE 'failed to execute message[^"]*' | head -1 >&2 || true
        return 1
    fi
    echo "$tx"
}

wait_for_inclusion() {
    local tx="$1"
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        sleep 2
        local q
        q=$($BINARY query tx "$tx" --node "$RPC_URL" -o json 2>/dev/null) || continue
        local code
        code=$(echo "$q" | jq -r '.code // empty')
        if [[ -n "$code" ]]; then
            echo "$q"
            return 0
        fi
    done
    return 1
}

read_expiry() {
    $BINARY query wasm contract-state smart "$AIRDROP" \
        "{\"airdrop\":{\"id\":$1}}" --node "$RPC_URL" -o json \
        | jq -r '.data.expiry_time'
}

upd_expiry_msg() {
    jq -nc --argjson i "$1" --argjson e "$2" \
        '{update_expiry:{airdrop_id:$i,expiry_time:$e}}'
}

# ─── Provision a fresh contract ─────────────────────────────────────────────

echo "📦 Storing fresh airdrop wasm..."
TX=$($BINARY tx wasm store "$WASM_PATH" --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" --node "$RPC_URL" \
    --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y 2>&1 \
    | awk '/^txhash:/{print $2}' | head -1)
echo "  store tx=$TX"
Q=$(wait_for_inclusion "$TX")
CODE_ID=$(echo "$Q" | jq -r '(.. | select(.type? == "store_code")? | .attributes[] | select(.key == "code_id") | .value) // empty' | head -1)
[[ -n "$CODE_ID" ]] || { echo "❌ no code_id from store tx"; exit 1; }
echo "  code_id=$CODE_ID"

echo "🎬 Instantiating..."
TX=$($BINARY tx wasm instantiate "$CODE_ID" "{\"admin\":\"$ADMIN\"}" \
    --label "Airdrop UpdateExpiry test $(date +%s)" --admin "$ADMIN" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y 2>&1 \
    | awk '/^txhash:/{print $2}' | head -1)
echo "  inst tx=$TX"
Q=$(wait_for_inclusion "$TX")
AIRDROP=$(echo "$Q" | jq -r '(.. | select(.type? == "instantiate")? | .attributes[] | select(.key == "_contract_address") | .value) // empty' | head -1)
[[ -n "$AIRDROP" ]] || { echo "❌ no contract address"; exit 1; }
echo "  contract=$AIRDROP"

# ─── Provision two seasons ───────────────────────────────────────────────────
#
# Season A: far-future start — used for pre-start T1..T4 tests.
# Season B: starts +90 s (short window) so we can wait for it to elapse and
#           then run the mid-vesting allowance test (T5).

NOW=$(date +%s)

A_START=$((NOW + 900))
A_VSTART=$((A_START))
A_VEND=$((A_START + 1800))
A_EXP=$((A_VEND + 1200))

B_OFFSET=90
B_START=$((NOW + B_OFFSET))
B_VSTART=$((B_START))
B_VEND=$((B_START + 1800))
B_EXP=$((B_VEND + 1200))

echo
echo "🎬 Adding Season A (start +900 s) ..."
MSG_A=$(jq -nc \
    --argjson s  "$A_START"  --argjson vs "$A_VSTART" \
    --argjson ve "$A_VEND"   --argjson e  "$A_EXP" \
    --arg addr "$ADMIN" \
    '{add_airdrop:{token_address:"uzig",is_native:true,
      start_time:$s,vesting_start_time:$vs,vesting_end_time:$ve,
      initial_claim_percent:40,apy_bonus_bps:0,
      eligibility:[[$addr,"1000000"]],expiry_time:$e}}')
TX=$(broadcast "$KEY_NAME" "$MSG_A")
echo "  season A add tx=$TX"
wait_for_inclusion "$TX" >/dev/null
A_ID=$($BINARY query wasm contract-state smart "$AIRDROP" '{"latest_airdrop_id":{}}' \
    --node "$RPC_URL" -o json | jq -r '.data.id')

echo "🎬 Adding Season B (start +${B_OFFSET} s) ..."
MSG_B=$(jq -nc \
    --argjson s  "$B_START"  --argjson vs "$B_VSTART" \
    --argjson ve "$B_VEND"   --argjson e  "$B_EXP" \
    --arg addr "$ADMIN" \
    '{add_airdrop:{token_address:"uzig",is_native:true,
      start_time:$s,vesting_start_time:$vs,vesting_end_time:$ve,
      initial_claim_percent:40,apy_bonus_bps:0,
      eligibility:[[$addr,"500000"]],expiry_time:$e}}')
TX=$(broadcast "$KEY_NAME" "$MSG_B")
echo "  season B add tx=$TX"
wait_for_inclusion "$TX" >/dev/null
B_ID=$($BINARY query wasm contract-state smart "$AIRDROP" '{"latest_airdrop_id":{}}' \
    --node "$RPC_URL" -o json | jq -r '.data.id')

echo "  season A id=$A_ID  season B id=$B_ID"

# ─── Pre-start matrix (Season A) ────────────────────────────────────────────

note "T1: pre-start, extend expiry further into future"
NEW_EXP=$((A_EXP + 3600))   # push 1 h beyond current expiry
if TX=$(broadcast "$KEY_NAME" "$(upd_expiry_msg "$A_ID" "$NEW_EXP")"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" ]]; then
        STORED=$(read_expiry "$A_ID")
        if [[ "$STORED" == "$NEW_EXP" ]]; then
            pass "expiry_time stored as $NEW_EXP"
        else
            fail "stored expiry=$STORED, expected $NEW_EXP"
        fi
    else
        fail "tx code=$(echo "$Q" | jq -r '.code') — $(echo "$Q" | jq -r '.raw_log // empty')"
    fi
else
    fail "unexpected rejection"
fi
A_EXP=$NEW_EXP   # update local variable to current stored state

note "T2: NonForwardUpdate — shrink expiry (less than current)"
SHRUNK_EXP=$((A_EXP - 600))
ERR=$(broadcast "$KEY_NAME" "$(upd_expiry_msg "$A_ID" "$SHRUNK_EXP")" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "NonForwardUpdate\|not a forward update\|must be greater" \
    && pass "rejected with NonForwardUpdate (shrink attempt)" \
    || fail "unexpected: $ERR"

note "T3: NonForwardUpdate — equal to current expiry"
ERR=$(broadcast "$KEY_NAME" "$(upd_expiry_msg "$A_ID" "$A_EXP")" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "NonForwardUpdate\|not a forward update\|must be greater" \
    && pass "rejected with NonForwardUpdate (equal value)" \
    || fail "unexpected: $ERR"

note "T4: non-admin caller ($NON_ADMIN_KEY)"
ANOTHER_EXP=$((A_EXP + 7200))
ERR=$(broadcast "$NON_ADMIN_KEY" "$(upd_expiry_msg "$A_ID" "$ANOTHER_EXP")" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "Unauthorized" \
    && pass "rejected with Unauthorized" \
    || fail "unexpected: $ERR"

# ─── Mid-vesting (Season B) — UpdateExpiry still allowed ────────────────────

# Wait until Season B has started (chain time > B_START) so we're in the
# vesting window, then verify UpdateExpiry is still accepted.
NOW=$(date +%s)
TARGET=$((B_START + 15))
if (( NOW < TARGET )); then
    WAIT=$((TARGET - NOW))
    echo
    echo "⏳ waiting ${WAIT}s for Season B to enter vesting phase..."
    sleep "$WAIT"
fi

note "T5: mid-vesting, extend expiry (permitted before current expiry passes)"
MID_NEW_EXP=$((B_EXP + 3600))
if TX=$(broadcast "$KEY_NAME" "$(upd_expiry_msg "$B_ID" "$MID_NEW_EXP")"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" ]]; then
        STORED=$(read_expiry "$B_ID")
        if [[ "$STORED" == "$MID_NEW_EXP" ]]; then
            pass "mid-vesting expiry extended to $MID_NEW_EXP"
        else
            fail "stored expiry=$STORED, expected $MID_NEW_EXP"
        fi
    else
        fail "tx code=$(echo "$Q" | jq -r '.code') — $(echo "$Q" | jq -r '.raw_log // empty')"
    fi
else
    fail "unexpected rejection"
fi

# ─── Summary ────────────────────────────────────────────────────────────────

echo
echo "═════════════════════════════════════"
echo "  pass: $PASS    fail: $FAIL"
echo "  contract: $AIRDROP (code_id $CODE_ID)"
echo "═════════════════════════════════════"
[[ "$FAIL" -eq 0 ]] || exit 1
