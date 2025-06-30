#!/usr/bin/env bash
set -euo pipefail

# ─── Load Devnet environment variables ────────────────────────────────────────
# Assumes ../devnet.env defines:
#   BINARY, RPC_URL, CHAIN_ID, KEY_NAME, KEYRING_BACKEND,
#   ROUTER_CONTRACT, ZIG_ADDRESS, NATIVE_A, NATIVE_B,
#   GAS_PRICES_NATIVE, GAS_ADJUSTMENT, FEES_NATIVE, BLOCK_TIME
source ../devnet.env

# ─── Helper: Get admin address (your own account) ─────────────────────────────
ADMIN_ADDRESS=$($BINARY keys show "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" -a)

# ─── Offer / Ask Denoms & Amounts ────────────────────────────────────────────
# Single-hop:
OFFER_AMOUNT_SINGLE="1000000"          # e.g., 1 ZIG (if 6 decimals)
OFFER_DENOM_SINGLE="uzig"
ASK_DENOM_SINGLE="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.devnet101"

# Two-hop:
OFFER_AMOUNT_TWO="1000000"             # e.g., 2 DENOM_HOP1
DENOM_HOP1="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.udev1"
ZIG_ADDRESS="uzig"
DENOM_HOP2="coin.zig18sytwc03z5j3wge5egf4rdue6gxkzzyf4658vq.devnet103"

# ─── 1. Query Router Configuration ──────────────────────────────────────────
query_config() {
  echo "Using Router at: $ROUTER_CONTRACT"
  echo "Admin address: $ADMIN_ADDRESS"
  echo
  echo ">> Query Router config"
  local payload='{"config":{}}'
  $BINARY query wasm contract-state smart "$ROUTER_CONTRACT" \
    "$payload" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    -o json | jq
  echo
  exit 0
}

# ─── 2. Simulate Single-Hop (ZIG → NATIVE_A) ─────────────────────────────────
simulate_single() {
  echo ">> Simulating single-hop swap: Offer ${OFFER_AMOUNT_SINGLE}${OFFER_DENOM_SINGLE} → ${ASK_DENOM_SINGLE}"
  payload="$(cat <<EOF
{
  "simulate_swap_operations": {
    "offer_amount": "${OFFER_AMOUNT_SINGLE}",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${OFFER_DENOM_SINGLE}" } },
          "ask_asset_info":   { "native_token": { "denom": "${ASK_DENOM_SINGLE}" } },
          "pair_type": { "xyk": {} }
        }
      }
    ]
  }
}
EOF
)"
  $BINARY query wasm contract-state smart "$ROUTER_CONTRACT" \
    "$payload" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    -o json | jq
  echo
  exit 0
}

# ─── 3. Reverse-Simulate Single-Hop (ZIG ← NATIVE_A) ──────────────────────────
reverse_simulate_single() {
  echo ">> Reverse-simulating single-hop swap: Want ${OFFER_AMOUNT_SINGLE}${ASK_DENOM_SINGLE} ← ${OFFER_DENOM_SINGLE}"
  payload="$(cat <<EOF
{
  "reverse_simulate_swap_operations": {
    "ask_amount": "${OFFER_AMOUNT_SINGLE}",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${OFFER_DENOM_SINGLE}" } },
          "ask_asset_info":   { "native_token": { "denom": "${ASK_DENOM_SINGLE}" } },
          "pair_type": { "xyk": {} }
        }
      }
    ]
  }
}
EOF
)"
  $BINARY query wasm contract-state smart "$ROUTER_CONTRACT" \
    "$payload" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    -o json | jq
  echo
  exit 0
}

# ─── 4. Execute Single-Hop Swap (ZIG → NATIVE_A) ──────────────────────────────
execute_single() {
  echo ">> Executing single-hop swap: Offer ${OFFER_AMOUNT_SINGLE}${OFFER_DENOM_SINGLE} → ${ASK_DENOM_SINGLE} (max_spread=0.01)"
  msg="$(cat <<EOF
{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${OFFER_DENOM_SINGLE}" } },
          "ask_asset_info":   { "native_token": { "denom": "${ASK_DENOM_SINGLE}" } },
          "pair_type": { "xyk": {} }
        }
      }
    ],
    "minimum_receive": "9455079",
    "to": "${ADMIN_ADDRESS}",
    "max_spread": "0.01"
  }
}
EOF
)"
  
  echo "📤 Sending message:"
  echo "$msg" | jq .
  echo
  
  $BINARY tx wasm execute "$ROUTER_CONTRACT" "$msg" \
    --amount "${OFFER_AMOUNT_SINGLE}${OFFER_DENOM_SINGLE}" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES" \
    -y -o json | jq
  echo
  exit 0
}

# ─── 5. Simulate Two-Hop (ZIG → NATIVE_A → NATIVE_B) ─────────────────────────
simulate_two() {
  echo ">> Simulating two-hop swap: Offer ${OFFER_AMOUNT_TWO}${DENOM_HOP1} → ${ZIG_ADDRESS} → ${DENOM_HOP2}"
  payload="$(cat <<EOF
{
  "simulate_swap_operations": {
    "offer_amount": "${OFFER_AMOUNT_TWO}",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${DENOM_HOP1}" } },
          "ask_asset_info":   { "native_token": { "denom": "${ZIG_ADDRESS}" } },
          "pair_type": { "xyk": {} }
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${ZIG_ADDRESS}" } },
          "ask_asset_info":   { "native_token": { "denom": "${DENOM_HOP2}" } },
          "pair_type": { "xyk": {} }
        }
      }
    ]
  }
}
EOF
)"
  $BINARY query wasm contract-state smart "$ROUTER_CONTRACT" \
    "$payload" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    -o json | jq
  echo
  exit 0
}

# ─── 6. Reverse-Simulate Two-Hop (ZIG ← NATIVE_B via NATIVE_A) ───────────────
reverse_simulate_two() {
  echo ">> Reverse-simulating two-hop swap: Want ${OFFER_AMOUNT_TWO}${DENOM_HOP2} ← ${DENOM_HOP1} ← ${ZIG_ADDRESS}"
  payload="$(cat <<EOF
{
  "reverse_simulate_swap_operations": {
    "ask_amount": "${OFFER_AMOUNT_TWO}",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${ZIG_ADDRESS}" } },
          "ask_asset_info":   { "native_token": { "denom": "${DENOM_HOP1}" } },
          "pair_type": { "xyk": {} }
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${DENOM_HOP1}" } },
          "ask_asset_info":   { "native_token": { "denom": "${DENOM_HOP2}" } },
          "pair_type": { "xyk": {} }
        }
      }
    ]
  }
}
EOF
)"
  $BINARY query wasm contract-state smart "$ROUTER_CONTRACT" \
    "$payload" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    -o json | jq
  echo
  exit 0
}

# ─── 7. Execute Two-Hop Swap (ZIG → NATIVE_A → NATIVE_B) ─────────────────────
execute_two() {
  echo ">> Executing two-hop swap: Offer ${OFFER_AMOUNT_TWO}${DENOM_HOP1} → ${ZIG_ADDRESS}→ ${DENOM_HOP2} (max_spread=0.01)"
  msg="$(cat <<EOF
{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${DENOM_HOP1}" } },
          "ask_asset_info":   { "native_token": { "denom": "${ZIG_ADDRESS}" } },
          "pair_type": { "xyk": {} }
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": { "native_token": { "denom": "${ZIG_ADDRESS}" } },
          "ask_asset_info":   { "native_token": { "denom": "${DENOM_HOP2}" } },
          "pair_type": { "xyk": {} }
        }
      }
    ],
    "minimum_receive": "9890",
    "to": "${ADMIN_ADDRESS}",
    "max_spread": "0.01"
  }
}
EOF
)"
  
  echo "📤 Sending message:"
  echo "$msg" | jq .
  echo
  
  $BINARY tx wasm execute "$ROUTER_CONTRACT" "$msg" \
    --amount "${OFFER_AMOUNT_TWO}${DENOM_HOP1}" \
    --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
    --node "$RPC_URL" --chain-id "$CHAIN_ID" \
    --gas auto --gas-adjustment "$GAS_ADJUSTMENT" --fees "$FEES" \
    -y -o json | jq
  echo
  exit 0
}

# ─── Dispatch ────────────────────────────────────────────────────────────────
if [[ $# -eq 0 ]]; then
  echo "Usage:"
  echo "  $0 query_config"
  echo "  $0 simulate_single"
  echo "  $0 reverse_simulate_single"
  echo "  $0 execute_single"
  echo "  $0 simulate_two"
  echo "  $0 reverse_simulate_two"
  echo "  $0 execute_two"
  exit 1
fi

case "$1" in
  query_config)
    query_config
    ;;
  simulate_single)
    simulate_single
    ;;
  reverse_simulate_single)
    reverse_simulate_single
    ;;
  execute_single)
    execute_single
    ;;
  simulate_two)
    simulate_two
    ;;
  reverse_simulate_two)
    reverse_simulate_two
    ;;
  execute_two)
    execute_two
    ;;
  *)
    echo "Unknown command: $1"
    exit 1
    ;;
esac
