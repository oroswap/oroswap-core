#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://devnet-rpc.zigchain.com"
CHAIN_ID="zig-devnet-1"
KEY_NAME="devnet-key"
KEYRING_BACKEND="test"
FACTORY_CONTRACT="zig1l8qk8d95jfhehz5pvevlad6rvlwvu87wfc9une07j6uua2ean7ns7d7heh"
ASSET1="uzig"
ASSET2="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.elon101"
CREATION_FEE="101000000uzig"
SLEEP_TIME=5
GAS_PRICES="0.25uzig"
GAS_ADJUSTMENT="1.3"
FEES="1000uzig"

# ─── 1) Submit create_pair ─────────────────────────────────────────────────
echo "⏳ Submitting create_pair to factory $FACTORY_CONTRACT..."
TXHASH=$($BINARY tx wasm execute "$FACTORY_CONTRACT" \
  '{
    "create_pair": {
      "pair_type": {"custom": "xyk_30"},
      "asset_infos": [
        {"native_token": {"denom": "'"$ASSET2"'"}},
        {"native_token": {"denom": "'"$ASSET1"'"}}
      ],
      "init_params": null
    }
  }' \
  --amount "$CREATION_FEE" \
  --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES" \
  --broadcast-mode sync \
  -y -o json \
  | jq -r .txhash)

echo "✅ Tx submitted: $TXHASH"

# ─── 2) Wait for inclusion ─────────────────────────────────────────────────
echo "⏳ Waiting $SLEEP_TIME seconds for block inclusion..."
sleep "$SLEEP_TIME"

# ─── 3) Query for full logs ─────────────────────────────────────────────────
RAW=$($BINARY query tx "$TXHASH" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json)

# ─── 4) Extract pair address ────────────────────────────────────────────────
PAIR_ADDR=$(echo "$RAW" | jq -r '
  .events[]
  | select(.type=="instantiate")
  | .attributes[]
  | select(.key=="_contract_address")
  | .value
')

echo

echo "🎉 New pair contract address: $PAIR_ADDR"
