#!/usr/bin/env bash
set -euo pipefail

# ─── Load your env (devnet.env) ───────────────────────────────────────────────
source ../devnet.env

# ─── Maker contract address (fee_address) ─────────────────────────────────────
MAKER_CONTRACT="zig1e8vp80sdczunxv00rlusu7lmmers0tg0tmfjejwl6n3ad8etk00qm2u0nw"

# Collect both devnet101 and devnet102 tokens and convert to uzig
COLLECT_MSG='{
  "collect": {
    "assets": [
      {
        "info": {
          "native_token": {
            "denom": "coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.devnet101"
          }
        },
        "limit": null
      },
      {
        "info": {
          "native_token": {
            "denom": "coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.devnet102"
          }
        },
        "limit": null
      }
    ]
  }
}'

echo "📤 Submitting collect transaction for devnet101 and devnet102 tokens..."
$BINARY tx wasm execute "$MAKER_CONTRACT" "$COLLECT_MSG" \
  --from "$KEY_NAME" \
  --chain-id "$CHAIN_ID" \
  --fees "20000uzig" \
  --gas auto \
  --gas-adjustment "$GAS_ADJUSTMENT" \
  --node "$RPC_URL" \
  --keyring-backend "$KEYRING_BACKEND" \
  -y -o json | jq

echo "⏳ Sleeping $SLEEP_TIME seconds for block inclusion..."
sleep "$SLEEP_TIME"

echo "🔍 Final Maker contract balances:"
$BINARY query bank balances "$MAKER_CONTRACT" \
  --node "$RPC_URL" \
  --chain-id "$CHAIN_ID" \
  -o json | jq