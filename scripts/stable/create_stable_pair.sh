#!/usr/bin/env bash
set -euo pipefail

# Load environment variables
source ../devnet.env

# Configuration
BINARY="$BINARY"
RPC_URL="$RPC_URL"
CHAIN_ID="$CHAIN_ID"
KEY_NAME="$KEY_NAME"
KEYRING_BACKEND="$KEYRING_BACKEND"
FACTORY="$FACTORY_CONTRACT"

# Transaction settings
TX_FEES="$FEES"
GAS_ADJUSTMENT="$GAS_ADJUSTMENT"

ADMIN=$(zigchaind keys show devnet-key -a --keyring-backend test)

# Stable pair parameters
STABLE_AMP=100  # Amplification parameter for stable pool

# ─── Functions ──────────────────────────────────────────────────────────────

# Create stable pair with native tokens
# Usage: create_stable_pair_native <asset1_denom> <asset2_denom> [amp]
create_stable_pair_native() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 create_stable_pair_native <asset1_denom> <asset2_denom> [amp]"; echo "Example: $0 create_stable_pair_native uzig uusd 100"; exit 1; }
  
  local asset1_denom="$1"
  local asset2_denom="$2"
  local amp="${3:-$STABLE_AMP}"
  
  echo "🏗️ Creating stable pair: $asset1_denom ↔ $asset2_denom (amp: $amp)"
  
  # Create init params for stable pool
  local init_params=$(cat <<EOF
{
  "amp": $amp,
  "owner": null
}
EOF
)
  
  # Encode init params to base64
  local encoded_params=$(printf '%s' "$init_params" | base64)
  
  # Debug: show the encoded params
  echo "🔍 Debug: Encoded params: $encoded_params"
  
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"create_pair\": {
        \"pair_type\": { \"stable\": {} },
        \"asset_infos\": [
          {\"native_token\": {\"denom\": \"$asset1_denom\"}},
          {\"native_token\": {\"denom\": \"$asset2_denom\"}}
        ],
        \"init_params\": \"$encoded_params\"
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    --amount 101000000uzig \
    -y -o json | jq
}

# Create stable pair with one native and one CW20 token
# Usage: create_stable_pair_mixed <native_denom> <cw20_contract_addr> [amp]
create_stable_pair_mixed() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 create_stable_pair_mixed <native_denom> <cw20_contract_addr> [amp]"; echo "Example: $0 create_stable_pair_mixed uzig zig1abc123 100"; exit 1; }
  
  local native_denom="$1"
  local cw20_contract="$2"
  local amp="${3:-$STABLE_AMP}"
  
  echo "🏗️ Creating stable pair: $native_denom ↔ $cw20_contract (amp: $amp)"
  
  # Create init params for stable pool
  local init_params=$(cat <<EOF
{
  "amp": $amp,
  "owner": null
}
EOF
)
  
  # Encode init params to base64
  local encoded_params=$(printf '%s' "$init_params" | base64)
  
  # Debug: show the encoded params
  echo "🔍 Debug: Encoded params: $encoded_params"
  
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"create_pair\": {
        \"pair_type\": { \"stable\": {} },
        \"asset_infos\": [
          {\"native_token\": {\"denom\": \"$native_denom\"}},
          {\"token\": {\"contract_addr\": \"$cw20_contract\"}}
        ],
        \"init_params\": \"$encoded_params\"
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    --amount 1000000uzig \
    -y -o json | jq
}

# Create stable pair with two CW20 tokens
# Usage: create_stable_pair_cw20 <cw20_contract1> <cw20_contract2> [amp]
create_stable_pair_cw20() {
  [ "$#" -ge 2 ] || { echo "Usage: $0 create_stable_pair_cw20 <cw20_contract1> <cw20_contract2> [amp]"; echo "Example: $0 create_stable_pair_cw20 zig1abc123 zig1def456 100"; exit 1; }
  
  local cw20_contract1="$1"
  local cw20_contract2="$2"
  local amp="${3:-$STABLE_AMP}"
  
  echo "🏗️ Creating stable pair: $cw20_contract1 ↔ $cw20_contract2 (amp: $amp)"
  
  # Create init params for stable pool
  local init_params=$(cat <<EOF
{
  "amp": $amp,
  "owner": null
}
EOF
)
  
  # Encode init params to base64
  local encoded_params=$(printf '%s' "$init_params" | base64)
  
  # Debug: show the encoded params
  echo "🔍 Debug: Encoded params: $encoded_params"
  
  $BINARY tx wasm execute "$FACTORY" \
    "{
      \"create_pair\": {
        \"pair_type\": { \"stable\": {} },
        \"asset_infos\": [
          {\"token\": {\"contract_addr\": \"$cw20_contract1\"}},
          {\"token\": {\"contract_addr\": \"$cw20_contract2\"}}
        ],
        \"init_params\": \"$encoded_params\"
      }
    }" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$TX_FEES" \
    --amount 1000000uzig \
    -y -o json | jq
}

# ─── Usage ──────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Commands:"
  echo "  create_stable_pair_native <asset1> <asset2> [amp]     - Create stable pair with native tokens"
  echo "  create_stable_pair_mixed <native> <cw20> [amp]        - Create stable pair with native + CW20"
  echo "  create_stable_pair_cw20 <cw20_1> <cw20_2> [amp]       - Create stable pair with two CW20 tokens"
  echo ""
  echo "Examples:"
  echo "  $0 create_stable_pair_native uzig uusd 100"
  echo "  $0 create_stable_pair_mixed uzig zig1abc123 100"
  echo "  $0 create_stable_pair_cw20 zig1abc123 zig1def456 100"
  echo ""
  echo "Note: Pool creation fee of 1,000,000 uzig will be charged for each pair creation"
  exit 0
fi

case "$1" in
  create_stable_pair_native)    shift; create_stable_pair_native "$@" ;;
  create_stable_pair_mixed)     shift; create_stable_pair_mixed "$@" ;;
  create_stable_pair_cw20)      shift; create_stable_pair_cw20 "$@" ;;
  *) echo "Unknown command: $1"; exit 1 ;;
esac
