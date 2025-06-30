#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://testnet-rpc.zigchain.com"
CHAIN_ID="zig-test-2"
KEY_NAME=""
KEYRING_BACKEND=""

# 🚨 Replace with the testnet‐deployed factory contract address:
FACTORY_CONTRACT="zig1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# Native token denom on testnet:
ASSET1="coin.<CREATOR_ADDRESS>.<SUB_DENOM>"

#IMPORTANT::Always keep uzig as the second Asset for testnet. 
ASSET2="uzig"

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
      "pair_type": {"xyk": {}},
      "asset_infos": [
        {"native_token": {"denom": "'"$ASSET1"'"}},
        {"native_token": {"denom": "'"$ASSET2"'"}}
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
