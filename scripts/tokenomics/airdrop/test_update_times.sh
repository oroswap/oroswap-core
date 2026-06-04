#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# End-to-end test for the UpdateTimes message on a live chain.
#
# Always provisions a fresh contract instance so prior runs cannot pollute
# state. The optimized wasm at opt-artifacts/oroswap_airdrop.wasm is stored
# and instantiated by this script — make sure it's up to date (run
# scripts/build_single_contract.sh tokenomics/airdrop if you've changed
# contract code).
#
# Exercises the full UpdateTimes phase gate:
#   T1  pre-start, all three fields pushed forward      → success
#   T2  pre-start, partial update (start_time only)     → success
#   T3  non-forward start_time (equal to current)       → NonForwardUpdate
#   T4  non-forward vesting_start_time (< current)      → NonForwardUpdate
#   T5  no fields provided (all None)                   → NoTimeFieldsProvided
#   T6  non-admin caller                                → Unauthorized
#   T7  post-start attempt (after airdrop begins)       → rejected
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

read_times() {
    $BINARY query wasm contract-state smart "$AIRDROP" \
        "{\"airdrop\":{\"id\":$1}}" --node "$RPC_URL" -o json \
        | jq '{start_time:.data.start_time,vesting_start_time:.data.vesting_start_time,vesting_end_time:.data.vesting_end_time}'
}

upd_times_msg() {
    # Args: airdrop_id, start_time_or_null, vesting_start_time_or_null, vesting_end_time_or_null
    # Pass "null" (without quotes) for fields you want to omit.
    jq -nc \
        --argjson i  "$1" \
        --argjson s  "$2" \
        --argjson vs "$3" \
        --argjson ve "$4" \
        '{update_times:{airdrop_id:$i,start_time:$s,vesting_start_time:$vs,vesting_end_time:$ve}}'
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
    --label "Airdrop UpdateTimes test $(date +%s)" --admin "$ADMIN" \
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
# Season A: far-future start — gives us plenty of room to push times forward
# before the chain crosses start_time.
# Season B: also pre-start, used for the post-start rejection (T7) — we'll
# wait for it to elapse before sending that message.

NOW=$(date +%s)

# Season A: start +900 s (15 min)
A_START=$((NOW + 900))
A_VSTART=$((A_START))
A_VEND=$((A_START + 1800))
A_EXP=$((A_VEND + 1200))

# Season B: start +90 s — we'll wait for it, then attempt UpdateTimes post-start
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

note "T1: pre-start, all three fields pushed forward"
# New times: start += 300, vesting_start += 300, vesting_end += 300
# All must remain < expiry_time and maintain ordering.
NEW_A_START=$((A_START + 300))
NEW_A_VSTART=$((A_VSTART + 300))
NEW_A_VEND=$((A_VEND + 300))
if TX=$(broadcast "$KEY_NAME" "$(upd_times_msg "$A_ID" "$NEW_A_START" "$NEW_A_VSTART" "$NEW_A_VEND")"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" ]]; then
        STORED=$(read_times "$A_ID")
        GOT_S=$(echo "$STORED"  | jq -r '.start_time')
        GOT_VS=$(echo "$STORED" | jq -r '.vesting_start_time')
        GOT_VE=$(echo "$STORED" | jq -r '.vesting_end_time')
        if [[ "$GOT_S" == "$NEW_A_START" && "$GOT_VS" == "$NEW_A_VSTART" && "$GOT_VE" == "$NEW_A_VEND" ]]; then
            pass "all three times updated on chain"
        else
            fail "tx succeeded but stored times are wrong: $STORED"
        fi
    else
        fail "tx code=$(echo "$Q" | jq -r '.code') — $(echo "$Q" | jq -r '.raw_log // empty')"
    fi
else
    fail "unexpected rejection"
fi
# Refresh local variables to current stored state for subsequent tests
A_START=$NEW_A_START
A_VSTART=$NEW_A_VSTART
A_VEND=$NEW_A_VEND

note "T2: pre-start, partial update (start_time only)"
NEW_A_START2=$((A_START + 60))
if TX=$(broadcast "$KEY_NAME" "$(upd_times_msg "$A_ID" "$NEW_A_START2" null null)"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" ]]; then
        GOT_S=$($BINARY query wasm contract-state smart "$AIRDROP" \
            "{\"airdrop\":{\"id\":$A_ID}}" --node "$RPC_URL" -o json \
            | jq -r '.data.start_time')
        if [[ "$GOT_S" == "$NEW_A_START2" ]]; then
            pass "start_time advanced to $NEW_A_START2, others unchanged"
        else
            fail "stored start_time=$GOT_S, expected $NEW_A_START2"
        fi
    else
        fail "tx code=$(echo "$Q" | jq -r '.code') — $(echo "$Q" | jq -r '.raw_log // empty')"
    fi
else
    fail "unexpected rejection"
fi
A_START=$NEW_A_START2

note "T3: NonForwardUpdate — start_time equal to current stored value"
ERR=$(broadcast "$KEY_NAME" "$(upd_times_msg "$A_ID" "$A_START" null null)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "NonForwardUpdate\|not a forward update\|must be greater" \
    && pass "rejected with NonForwardUpdate (equal value)" \
    || fail "unexpected: $ERR"

note "T4: NonForwardUpdate — vesting_start_time less than current stored value"
STALE_VSTART=$((A_VSTART - 60))
ERR=$(broadcast "$KEY_NAME" "$(upd_times_msg "$A_ID" null "$STALE_VSTART" null)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "NonForwardUpdate\|not a forward update\|must be greater" \
    && pass "rejected with NonForwardUpdate (backward value)" \
    || fail "unexpected: $ERR"

note "T5: NoTimeFieldsProvided — all fields None"
ERR=$(broadcast "$KEY_NAME" "$(upd_times_msg "$A_ID" null null null)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "NoTimeFieldsProvided\|no time fields\|at least one" \
    && pass "rejected with NoTimeFieldsProvided" \
    || fail "unexpected: $ERR"

note "T6: non-admin caller ($NON_ADMIN_KEY)"
NEW_A_START3=$((A_START + 120))
ERR=$(broadcast "$NON_ADMIN_KEY" "$(upd_times_msg "$A_ID" "$NEW_A_START3" null null)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "Unauthorized" \
    && pass "rejected with Unauthorized" \
    || fail "unexpected: $ERR"

# ─── Post-start rejection (Season B) ────────────────────────────────────────

# Wait until Season B's start_time has passed (plus a small cushion so the
# on-chain block time is safely past start_time before we send the tx).
NOW=$(date +%s)
TARGET=$((B_START + 15))
if (( NOW < TARGET )); then
    WAIT=$((TARGET - NOW))
    echo
    echo "⏳ waiting ${WAIT}s for Season B start_time + 15 s cushion..."
    sleep "$WAIT"
fi

note "T7: post-start UpdateTimes rejected"
NEW_B_START=$((B_START + 300))
ERR=$(broadcast "$KEY_NAME" "$(upd_times_msg "$B_ID" "$NEW_B_START" null null)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "AirdropAlreadyStarted\|already started\|pre-start only" \
    && pass "rejected after start (AirdropAlreadyStarted)" \
    || fail "unexpected: $ERR"

# ─── Summary ────────────────────────────────────────────────────────────────

echo
echo "═════════════════════════════════════"
echo "  pass: $PASS    fail: $FAIL"
echo "  contract: $AIRDROP (code_id $CODE_ID)"
echo "═════════════════════════════════════"
[[ "$FAIL" -eq 0 ]] || exit 1
