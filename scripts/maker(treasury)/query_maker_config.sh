#!/usr/bin/env bash
set -euo pipefail

# ─── Load your env (devnet.env) ───────────────────────────────────────────────
source ../devnet.env

# ─── Maker contract address (fee_address) ─────────────────────────────────────

echo "🔍 Querying Maker contract configuration..."
echo "Contract: $MAKER_CONTRACT"
echo ""

# Query the Maker contract config
$BINARY query wasm contract-state smart "$MAKER_CONTRACT" '{"config":{}}' \
  --node "$RPC_URL" \
  --chain-id "$CHAIN_ID" \
  -o json | jq '.data' 