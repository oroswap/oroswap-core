#!/usr/bin/env bash
set -euo pipefail

source ../devnet_testing.env

#Single admin setup 
ADMIN=$(zigchaind keys show devnet-key -a --keyring-backend test)


CHAIN_ID="zig-devnet-1"

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
    },
    {
      "code_id": '"$PAIR_CODE_ID"',
      "pair_type": { "custom": "xyk_30" },
      "permissioned": false,
      "total_fee_bps": 30,
      "maker_fee_bps": 2000,
      "is_disabled": false,
      "is_generator_disabled": false,
      "pool_creation_fee": "1000000"
    }
  ],
  "fee_address": "zig1df54xf69fq8h62tcaa42pxqemss9qa0f8339wh",
  "generator_address": null,
  "tracker_config": null
}' \
  --label "oroswap-factory" \
  --admin "$ADMIN" \
  --from devnet-key --keyring-backend test \
  --node https://devnet-rpc.zigchain.com \
  --chain-id "$CHAIN_ID" \
  --broadcast-mode sync \
  --gas auto --gas-adjustment 1.3 --gas-prices 0.25uzig \
  -y -o json | jq .

echo "✅ OroSwap Factory instantiation complete."
