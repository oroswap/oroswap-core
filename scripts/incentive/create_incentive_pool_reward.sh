#!/usr/bin/env bash
set -euo pipefail

# --- Load environment ---
source "../devnet.env"
# — the CW20‐share LP you registered
LP="coin.zig1xemzpkq2qd6a5e08xxy5ffcwx9r4xn5fqe6v02rkte883f9xhg5qhsevjy.oroswaplptoken"

# — your external (factory) token denom
REWARD_DENOM="factory/zig1w6ccfqzezwykkhgwmz4fyx60s3kxr4q7ew2wy7/token4"

AMOUNT="100000000"

zigchaind tx wasm execute $INC_CONTRACT '{
  "incentivize": {
    "lp_token": "'"$LP"'",
    "schedule": {
      "reward": {
        "info": { "native_token": { "denom":"'"$REWARD_DENOM"'" } },
        "amount":"'"$AMOUNT"'"
      },
      "duration_periods": 1
    }
  }
}' \
  --from z \
  --amount ${AMOUNT}${REWARD_DENOM} \
  --fees 5000uzig \
  --gas auto --gas-adjustment 1.3 \
  -y