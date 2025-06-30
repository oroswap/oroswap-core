#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------------
# Script: concentrated_pair.sh
# Purpose: Deploy a concentrated‐liquidity pair via Astroport factory on Devnet
# Usage: concentrated_pair.sh <denom1> <denom2>
# ------------------------------------------------------------------

# ─── Load Devnet environment variables ─────────────────────────────────────────
# Expects devnet.env to define:
#   BINARY, RPC_URL, CHAIN_ID, KEY_NAME, KEYRING_BACKEND, FACTORY_CONTRACT,
#   ASTROPORT_PAIR_CONCENTRATED_CODE_ID
source ../devnet.env

# ─── Defaults ─────────────────────────────────────────────────────────────────
FEES="5000uzig"
AMOUNT="101000000uzig"   # ← must cover the 100_000_000uzig denom‐creation fee

# ─── Derive "owner" (admin of new pair) ───────────────────────────────────────
OWNER=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")

# ─── Build init_params JSON and Base64‐encode ─────────────────────────────────
INIT_JSON=$(jq -nc '{
  amp:              "40.0",
  gamma:            "0.0001",
  mid_fee:          "0.005",
  out_fee:          "0.01",
  fee_gamma:        "0.001",
  repeg_profit_threshold: "0.0001",
  min_price_scale_delta:  "0.000001",
  price_scale:      "1.5",
  ma_half_time:     600,
  track_asset_balances: false
}')
INIT_BASE64=$(printf '%s' "$INIT_JSON" | base64 | tr -d '\n')

echo "INIT_PARAMS_JSON: $INIT_JSON"
echo "INIT_PARAMS_BASE64 (first 20 chars): ${INIT_BASE64:0:20}…"

# ─── Validate arguments ────────────────────────────────────────────────────────
if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <denom1> <denom2>"
  exit 1
fi
DENOM1=$1
DENOM2=$2

# ─── Build create_pair payload ────────────────────────────────────────────────
echo "Building payload for concentrated pair: $DENOM1 / $DENOM2"
PAYLOAD=$(
  jq -nc \
    --arg d1 "$DENOM1" \
    --arg d2 "$DENOM2" \
    --arg ip "$INIT_BASE64" \
  '{
    create_pair: {
      pair_type: { custom: "concentrated" },
      asset_infos: [
        { native_token: { denom: $d1 } },
        { native_token: { denom: $d2 } }
      ],
      init_params: $ip
    }
  }'
)
# <— DO NOT do "| jq" here, since $PAYLOAD contains base64
echo "Payload: $PAYLOAD"

# ─── Execute transaction and capture raw output ────────────────────────────────
set +e
RAW=$(
  $BINARY tx wasm execute "$FACTORY_CONTRACT" "$PAYLOAD" \
    --amount "$AMOUNT" \
    --from "$KEY_NAME" \
    --keyring-backend "$KEYRING_BACKEND" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --fees "$FEES" \
    --gas auto --gas-adjustment 1.3 \
    -y -o json 2>&1
)
RES=$?
set -e

if [[ $RES -eq 0 ]]; then
  TXHASH=$(echo "$RAW" | jq -r .txhash)
  echo "TX hash: $TXHASH"

  echo "⏳ Waiting a couple of seconds for block inclusion..."
  sleep 2

  ADDRESS=$(
    $BINARY query tx "$TXHASH" \
      --node "$RPC_URL" \
      --chain-id "$CHAIN_ID" -o json \
    | jq -r '
        .logs[].events[]
        | select(.type=="instantiate")
        | .attributes[]
        | select(.key=="_contract_address").value
      '
  )
  echo "✅ Concentrated pair deployed at: $ADDRESS"
  exit 0
fi

# ─── If we reach here, the transaction failed ───────────────────────────────────
echo "❌ Chain response (raw):"
echo "$RAW" | jq . 2>/dev/null || echo "$RAW"
echo "❌ raw_log:"
echo "$RAW" | jq -r .raw_log 2>/dev/null || echo "N/A"
exit 1
