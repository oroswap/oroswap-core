#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://devnet-rpc.zigchain.com"
CHAIN_ID="zig-devnet-1"
FACTORY="zig1wn625s4jcmvk0szpl85rj5azkfc6suyvf75q6vrddscjdphtve8sh9354q"

# ─── Inspect pairs ──────────────────────────────────────────────────────────

# echo "🔍 1) All registered pools (any type):"
# $BINARY query wasm contract-state smart "$FACTORY" \
#   '{"pairs":{}}' \
#   --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json \
# | jq

# echo
# echo "🔍 2) All XYK pools:"
# $BINARY query wasm contract-state smart "$FACTORY" \
#   '{"pairs":{"pair_type":{"xyk":{}},"start_after":null,"limit":100}}' \
#   --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json \
# | jq

# echo



echo "🔍 3) Pools for assets [kai, uzig]:"

zigchaind query wasm contract-state smart "$FACTORY" \
  '{
    "pairs_by_assets": {
      "asset_infos": [
        { "native_token": { "denom": "coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.trump101" } },
        { "native_token": { "denom": "uzig" } }
      ]
    }
  }' \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" -o json \
| jq
