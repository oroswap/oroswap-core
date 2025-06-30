#!/usr/bin/env bash
set -euo pipefail

# ─── CONFIG ───────────────────────────────────────────────────────────────────
FACTORY_CODE_ID=${FACTORY_CODE_ID:-37}    # your factory code_id on testnet
CW20_CODE_ID=${CW20_CODE_ID:-4}          # your CW20 token code_id on testnet
PAIR_CODE_ID=${PAIR_CODE_ID:-38}         # your Pair contract code_id on testnet

ADMIN=$(zigchaind keys show testnet-admin -a --keyring-backend file)
CHAIN_ID="zig-test-2"

# ─── INSTANTIATE ───────────────────────────────────────────────────────────────
echo "⏳ Instantiating OroSwap Factory (code_id=$FACTORY_CODE_ID)…"

zigchaind tx wasm instantiate "$FACTORY_CODE_ID" '{
  "owner": "'"$ADMIN"'", 
  "token_code_id": '"$CW20_CODE_ID"', 
  "whitelist_code_id": 0, 
  "coin_registry_address": "'"$ADMIN"'", 
  "pair_configs": [
    {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "xyk": {} },
      "permissioned": false,
      "total_fee_bps": 10,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  ],
  "fee_address": "zig1rzad9u9py87mzpgvryypdgdnw6yr22eer6dyzn",
  "generator_address": null,
  "tracker_config": null
}' \
  --label "oroswap-factory" \
  --admin "$ADMIN" \
  --from testnet-admin --keyring-backend file \
  --node https://testnet-rpc.zigchain.com \
  --chain-id "$CHAIN_ID" \
  --broadcast-mode sync \
  --gas auto --gas-adjustment 1.3 --gas-prices 0.25uzig \
  -y -o json | jq .

echo "✅ OroSwap Factory instantiation complete."
