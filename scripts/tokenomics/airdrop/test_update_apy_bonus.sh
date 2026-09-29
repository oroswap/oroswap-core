#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# End-to-end test for the UpdateApyBonus message on a live chain.
#
# Always provisions a fresh contract instance so prior runs cannot pollute
# state (eligibility totals, contract balance, etc.). The optimized wasm at
# opt-artifacts/oroswap_airdrop.wasm is stored and instantiated by this
# script — make sure it's up to date (run scripts/build_single_contract.sh
# tokenomics/airdrop if you've changed contract code).
#
# Exercises the full UpdateApyBonus phase gate:
#   T1  pre-start INCREASE (funded)           → success
#   T2  pre-start DECREASE                    → success
#   T3  bps > MAX_APY_BONUS_BPS               → ApyBonusTooHigh
#   T4  underfunded INCREASE                  → ApyUpdateUnderfunded
#   T5  non-admin caller                      → Unauthorized
#   T6  post-start DECREASE                   → ApyDecreaseNotAllowed
#   T7  post-start INCREASE (funded)          → success
#
# The post-vesting-end branch (ApyUpdateAfterVestingEnd) is covered by the
# unit test test_update_apy_bonus_after_vesting_end_rejected — replicating
# it live would require waiting through the full vesting window.
#
# Requirements:
#   • devnet_testing.env exports KEY_NAME, BINARY, RPC_URL, CHAIN_ID, gas vars
#   • opt-artifacts/oroswap_airdrop.wasm is built
#   • NON_ADMIN_KEY (default: unauthorized_test) — a separate key with gas
#     funds, used to verify the admin gate
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# Capture caller-provided overrides BEFORE sourcing the env file, since the
# env file also `export`s some of these and would otherwise clobber the caller.
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

read_apy() {
    $BINARY query wasm contract-state smart "$AIRDROP" \
        "{\"airdrop\":{\"id\":$1}}" --node "$RPC_URL" -o json \
        | jq -r '.data.apy_bonus_bps'
}

upd_msg() {
    jq -nc --argjson i "$1" --argjson n "$2" \
        '{update_apy_bonus:{airdrop_id:$i,apy_bonus_bps:$n}}'
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
    --label "Airdrop UpdateApyBonus test $(date +%s)" --admin "$ADMIN" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" --gas-prices "$GAS_PRICES" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" -y 2>&1 \
    | awk '/^txhash:/{print $2}' | head -1)
echo "  inst tx=$TX"
Q=$(wait_for_inclusion "$TX")
AIRDROP=$(echo "$Q" | jq -r '(.. | select(.type? == "instantiate")? | .attributes[] | select(.key == "_contract_address") | .value) // empty' | head -1)
[[ -n "$AIRDROP" ]] || { echo "❌ no contract address"; exit 1; }
echo "  contract=$AIRDROP"

# ─── Provision two seasons + funding ────────────────────────────────────────

# Eligibility numbers chosen so:
#   season A (eligibility 1_000_000, initial 40%, vesting_base 600_000):
#     • max_outstanding @ bps=2000 = 1_000_000 + 120_000 = 1_120_000  ← T1 funded
#     • max_outstanding @ bps=9000 = 1_000_000 + 540_000 = 1_540_000  ← T4 underfunded
#   We deposit 1_120_000 → T1 ok, T4 must fail.

NOW=$(date +%s)
SEASON_A_START=$((NOW + 900))        # 15 min — comfortably pre-start
SEASON_A_VEND=$((SEASON_A_START + 1800))
SEASON_A_EXP=$((SEASON_A_VEND + 1200))

# season B start time chosen so it elapses *after* we finish season A tests
SEASON_B_OFFSET=120                  # 2 min from now
SEASON_B_START=$((NOW + SEASON_B_OFFSET))
SEASON_B_VEND=$((SEASON_B_START + 1800))
SEASON_B_EXP=$((SEASON_B_VEND + 1200))

echo
echo "🎬 Adding Season A (start +900s) and Season B (start +${SEASON_B_OFFSET}s)..."
MSG_A=$(jq -nc \
    --argjson s "$SEASON_A_START" --argjson v "$SEASON_A_VEND" --argjson e "$SEASON_A_EXP" \
    --arg addr "$ADMIN" \
    '{add_airdrop:{token_address:"uzig",is_native:true,start_time:$s,vesting_start_time:$s,
      vesting_end_time:$v,initial_claim_percent:40,apy_bonus_bps:1000,
      eligibility:[[$addr,"1000000"]],expiry_time:$e}}')
TX=$(broadcast "$KEY_NAME" "$MSG_A")
echo "  season A add tx=$TX"
wait_for_inclusion "$TX" >/dev/null
A_ID=$($BINARY query wasm contract-state smart "$AIRDROP" '{"latest_airdrop_id":{}}' --node "$RPC_URL" -o json | jq -r '.data.id')

MSG_B=$(jq -nc \
    --argjson s "$SEASON_B_START" --argjson v "$SEASON_B_VEND" --argjson e "$SEASON_B_EXP" \
    --arg addr "$ADMIN" \
    '{add_airdrop:{token_address:"uzig",is_native:true,start_time:$s,vesting_start_time:$s,
      vesting_end_time:$v,initial_claim_percent:40,apy_bonus_bps:1000,
      eligibility:[[$addr,"500000"]],expiry_time:$e}}')
TX=$(broadcast "$KEY_NAME" "$MSG_B")
echo "  season B add tx=$TX"
wait_for_inclusion "$TX" >/dev/null
B_ID=$($BINARY query wasm contract-state smart "$AIRDROP" '{"latest_airdrop_id":{}}' --node "$RPC_URL" -o json | jq -r '.data.id')

echo "  season A id=$A_ID, season B id=$B_ID"

echo "💰 Depositing exactly 1_120_000 uzig (covers apy≤2000 but NOT 9000)..."
MSG_DEP='{"deposit_funds":{"token_address":"uzig","amount":"1120000","is_native":true}}'
TX=$(broadcast "$KEY_NAME" "$MSG_DEP" --amount 1120000uzig)
echo "  deposit tx=$TX"
wait_for_inclusion "$TX" >/dev/null

# ─── Pre-start matrix (Season A) ────────────────────────────────────────────

note "T1: pre-start INCREASE 1000 → 2000 (funded)"
if TX=$(broadcast "$KEY_NAME" "$(upd_msg "$A_ID" 2000)"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" && $(read_apy "$A_ID") == "2000" ]]; then
        pass "state=2000 on chain"
    else
        fail "tx included but state did not flip"
    fi
else
    fail "unexpected rejection"
fi

note "T2: pre-start DECREASE 2000 → 500"
if TX=$(broadcast "$KEY_NAME" "$(upd_msg "$A_ID" 500)"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" && $(read_apy "$A_ID") == "500" ]]; then
        pass "state=500 on chain (pre-start decrease ok)"
    else
        fail "tx included but state did not flip"
    fi
else
    fail "unexpected rejection"
fi

note "T3: ApyBonusTooHigh (bps=10001)"
ERR=$(broadcast "$KEY_NAME" "$(upd_msg "$A_ID" 10001)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "apy_bonus_bps too high" && pass "rejected with ApyBonusTooHigh" || fail "unexpected: $ERR"

note "T4: underfunded INCREASE 500 → 9000"
ERR=$(broadcast "$KEY_NAME" "$(upd_msg "$A_ID" 9000)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "APY update would exceed contract funding" && pass "rejected with ApyUpdateUnderfunded" || fail "unexpected: $ERR"

note "T5: non-admin caller ($NON_ADMIN_KEY)"
ERR=$(broadcast "$NON_ADMIN_KEY" "$(upd_msg "$A_ID" 1500)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "Unauthorized" && pass "rejected with Unauthorized" || fail "unexpected: $ERR"

# ─── Post-start matrix (Season B) ───────────────────────────────────────────

# Make sure we've crossed Season B's start_time AND given the chain a couple
# of blocks of cushion so the simulation/inclusion sees current_time > start.
NOW=$(date +%s)
TARGET=$((SEASON_B_START + 15))
if (( NOW < TARGET )); then
    WAIT=$((TARGET - NOW))
    echo
    echo "⏳ waiting ${WAIT}s for Season B start_time + 15s cushion..."
    sleep "$WAIT"
fi

note "T6: POST-start DECREASE 1000 → 500"
ERR=$(broadcast "$KEY_NAME" "$(upd_msg "$B_ID" 500)" 2>&1 1>/dev/null || true)
echo "$ERR" | grep -q "APY decrease not allowed" && pass "rejected with ApyDecreaseNotAllowed" || fail "unexpected: $ERR"

note "T7: POST-start INCREASE 1000 → 2000 (funded)"
# Season B: vesting_base = 500_000 * 60% = 300_000; max_bonus@2000 = 60_000;
# max_outstanding = 560_000. Plus Season A's outstanding (1_000_000) — but
# solvency is per-season, so the per-season check is what matters. Balance
# 1_120_000 covers Season B's 560_000 with headroom.
if TX=$(broadcast "$KEY_NAME" "$(upd_msg "$B_ID" 2000)"); then
    Q=$(wait_for_inclusion "$TX")
    if [[ $(echo "$Q" | jq -r '.code') == "0" && $(read_apy "$B_ID") == "2000" ]]; then
        pass "state=2000 (post-start increase ok)"
    else
        fail "tx included but state did not flip"
    fi
else
    fail "unexpected rejection"
fi

echo
echo "═════════════════════════════════════"
echo "  pass: $PASS    fail: $FAIL"
echo "  contract: $AIRDROP (code_id $CODE_ID)"
echo "═════════════════════════════════════"
[[ "$FAIL" -eq 0 ]] || exit 1
