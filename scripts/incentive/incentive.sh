#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------------
# Script: incentive.sh
# Purpose: Unified script for deploying and managing Oroswap incentives
# Usage: incentive.sh <command> [args]
# Requires: ../devnet.env to be present
# Commands:
#   instantiate-vesting
#   instantiate-incentives
#   setup-pools <LP> [alloc_points]
#   incentivize-external <LP> <DENOM> <AMOUNT> [duration_periods]
#   stake-lp <LP> <AMOUNT>
#   withdraw-lp <LP> <AMOUNT>
#   claim-rewards <LP>
#   fetch-rewards
#   update-config <ORO_TOKEN>
# ------------------------------------------------------------------

# --- Load environment ---
source "../devnet.env"

# --- Config from env ---
BINARY="${BINARY:-zigchaind}"
RPC_URL="${RPC_URL:?must be set in devnet.env}"
CHAIN_ID="${CHAIN_ID:?must be set in devnet.env}"
FEES="${FEES:?must be set in devnet.env}"
GAS_ADJUSTMENT="${GAS_ADJUSTMENT:-1.3}"

# Contracts & IDs from env
FACTORY="${FACTORY_CONTRACT:?must be set in devnet.env}"
ORO_TOKEN="${ZIG_ADDRESS:?must be set in devnet.env}"
#VESTING_CONTRACT="${VESTING_CONTRACT:?must be set in devnet.env}"              # address of existing vesting contract
#OROSWAP_VESTING_CODE_ID="${OROSWAP_VESTING_CODE_ID:?must be set in devnet.env}" # code ID for vesting (only used if instantiating vesting)

# define your admin/user wallet
ADMIN_WALLET=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")
USER="${USER:-$ADMIN_WALLET}"

# --- Helpers ---
usage() {
  cat <<EOF
Usage: $0 <command> [args]
Commands:
  instantiate-vesting
  instantiate-incentives
  setup-pools <LP> [alloc_points]
  incentivize-external <LP> <DENOM> <AMOUNT> [duration_periods]
  stake-lp <LP> <AMOUNT>
  withdraw-lp <LP> <AMOUNT>
  claim-rewards <LP>
  fetch-rewards   # list pending rewards for \$USER
  update-config <ORO_TOKEN>  # update incentives contract config with new oro_token
  update-incentivization-fee  # update incentivization fee to 1000000uzig
  show-config  # display current incentives contract configuration
EOF
  exit 1
}

instantiate_vesting() {
  echo "Instantiating vesting contract (code ID: $OROSWAP_VESTING_CODE_ID)..."
  TX=$(
    $BINARY tx wasm instantiate "$OROSWAP_VESTING_CODE_ID" '{
      "owner": "'"$ADMIN_WALLET"'",
      "vesting_token": { "native_token": { "denom": "'"$ORO_TOKEN"'" } }
    }' \
      --label "oro-vesting" \
      --admin "$ADMIN_WALLET" \
      --from "$KEY_NAME" \
      --chain-id "$CHAIN_ID" \
      --node "$RPC_URL" \
      --gas auto \
      --gas-adjustment "$GAS_ADJUSTMENT" \
      --fees "$FEES" \
      -y -o json | jq -r .txhash
  )
  echo "TX hash: $TX"
  NEW_ADDR=$(
    $BINARY query tx "$TX" \
      --node "$RPC_URL" \
      -o json \
      | jq -r '
          .events[]
          | select(.type=="instantiate")
          | .attributes[]
          | select(.key=="_contract_address")
          | .value
        '
  )
  echo "Vesting contract deployed at: $NEW_ADDR"
  echo
  echo "If you want to use this address for incentive instantiation later, set:"
  echo "  export VESTING_CONTRACT=\"$NEW_ADDR\""
}

instantiate_incentives() {
  echo "Instantiating incentives contract (code ID: $OROSWAP_INCENTIVES_CODE_ID)..."
  TX=$(
    $BINARY tx wasm instantiate "$OROSWAP_INCENTIVES_CODE_ID" '{
      "owner": "'"$ADMIN_WALLET"'",
      "factory": "'"$FACTORY"'",
      "oro_token": { "native_token": { "denom": "'"$ZIG_ADDRESS"'" } },
      "vesting_contract": "'"$ADMIN_WALLET"'",
      "incentivization_fee_info": {
        "fee_receiver": "'"$ADMIN_WALLET"'",
        "fee": {
          "amount": "1000000",
          "denom": "uzig"
        }
      }
    }' \
      --label "oro-incentives" \
      --admin "$ADMIN_WALLET" \
      --from "$KEY_NAME" \
      --chain-id "$CHAIN_ID" \
      --node "$RPC_URL" \
      --gas auto \
      --gas-adjustment "$GAS_ADJUSTMENT" \
      --fees "$FEES" \
      -y -o json | jq -r .txhash
  )
  echo "TX hash: $TX"
  NEW_ADDR=$(
    $BINARY query tx "$TX" \
      --node "$RPC_URL" \
      -o json \
      | jq -r '
          .events[]
          | select(.type=="instantiate")
          | .attributes[]
          | select(.key=="_contract_address")
          | .value
        '
  )
  echo "Incentives contract deployed at: $NEW_ADDR"
  echo
  echo "If you want to use this address for future commands, set:"
  echo "  export INC_CONTRACT=\"$NEW_ADDR\""
}

setup_pools() {
  LP="$1"; ALLOC="${2:-1}"
  echo "Registering pool $LP with alloc $ALLOC..."
  $BINARY tx wasm execute "$INC_CONTRACT" '{"setup_pools": {"pools": [["'"$LP"'","'"$ALLOC"'" ]]}}' \
    --from "$KEY_NAME" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    -y
}

incentivize_external() {
  LP="$1"; DENOM="$2"; AMT="$3"; DUR="${4:-1}"
  INCENTIVE_FEE="1000000"  # Fixed incentivization fee in uzig
  
  echo "Scheduling $AMT $DENOM to $LP over $DUR periods..."
  echo "Incentivization fee: ${INCENTIVE_FEE}uzig"
  echo "Reward amount: ${AMT}${DENOM}"
  echo "Total amount to send: ${INCENTIVE_FEE}uzig,${AMT}${DENOM}"
  
  $BINARY tx wasm execute "$INC_CONTRACT" '{
    "incentivize": {
      "lp_token": "'"$LP"'",
      "schedule": {
        "reward": { "info": { "native_token": { "denom": "'"$DENOM"'" } }, "amount": "'"$AMT"'" },
        "duration_periods": '"$DUR"'
      }
    }
  }' \
    --from "$KEY_NAME" \
    --amount "${INCENTIVE_FEE}uzig,${AMT}${DENOM}" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    -y
}

stake_lp() {
  LP="$1"; AMT="$2"
  echo "Staking $AMT $LP..."
  if [[ "$LP" == cw* ]]; then
    MSG=$(echo -n '{"deposit":{}}' | base64)
    $BINARY tx wasm execute "$LP" '{"send": {"contract": "'"$INC_CONTRACT"'","amount": "'"$AMT"'","msg": "'"$MSG"'"}}' \
      --from "$KEY_NAME" \
      --chain-id "$CHAIN_ID" \
      --node "$RPC_URL" \
      --gas auto \
      --gas-adjustment "$GAS_ADJUSTMENT" \
      --fees "$FEES" \
      -y
  else
    $BINARY tx wasm execute "$INC_CONTRACT" '{"deposit":{"recipient":null}}' \
      --from "$KEY_NAME" \
      --amount "${AMT}${LP}" \
      --chain-id "$CHAIN_ID" \
      --node "$RPC_URL" \
      --gas auto \
      --gas-adjustment "$GAS_ADJUSTMENT" \
      --fees "$FEES" \
      -y
  fi
}

withdraw_lp() {
  LP="$1"; AMT="$2"
  echo "Withdrawing $AMT $LP..."
  $BINARY tx wasm execute "$INC_CONTRACT" '{
    "withdraw": { "lp_token": "'"$LP"'", "amount": "'"$AMT"'" }
  }' \
    --from "$KEY_NAME" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    -y
}

claim_rewards() {
  LP="$1"
  echo "Claiming rewards for $LP..."
  $BINARY tx wasm execute "$INC_CONTRACT" '{"claim_rewards":{"lp_tokens":["'"$LP"'" ]}}' \
    --from "$KEY_NAME" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    -y
}

fetch_rewards() {
  echo "Fetching pending rewards for user $ADMIN_WALLET..."
  
  # Only check the specific pool where user has staked LP tokens
  LP_TOKEN="coin.zig1tqwwyth34550lg2437m05mjnjp8w7h5ka7m70jtzpxn4uh2ktsmqg4j0v6.oroswaplptoken"
  
  printf "%-60s | %s\n" "LP Token" "Pending Rewards"
  printf "%-60s-+-%s\n" "------------------------------------------------------------" "---------------"
  
  # Get pending rewards for this specific pool
  $BINARY query wasm contract-state smart "$INC_CONTRACT" '{"pending_rewards":{"lp_token":"'"$LP_TOKEN"'","user":"'"$ADMIN_WALLET"'"}}' \
    --node "$RPC_URL" \
    --chain-id "$CHAIN_ID" \
    -o json > /tmp/pending_rewards.json 2>&1
  
  PENDING_JSON=$(cat /tmp/pending_rewards.json)
  
  # Check if query was successful (contains "data" field and no error)
  if echo "$PENDING_JSON" | grep -q '"data"' && ! echo "$PENDING_JSON" | grep -q "doesn't have position"; then
    # Check if there are any rewards
    REWARD_COUNT=$(echo "$PENDING_JSON" | jq '.data | length')
    
    if [[ "$REWARD_COUNT" -gt 0 ]]; then
      # Extract and display each reward
      echo "$PENDING_JSON" | jq -r '.data[] | "\(.amount) \(.info.native_token.denom // .info.token.contract_addr)"' | while read -r amount token; do
        if [[ -n "$amount" && -n "$token" ]]; then
          printf "%-60s | %s %s\n" "$LP_TOKEN" "$amount" "$token"
        fi
      done
    fi
  fi
  rm -f /tmp/pending_rewards.json
}

# --- Update the update_config method to use ORO_TOKEN from devnet_testing.env ---
update_config() {
  echo "Updating incentives contract config with oro_token: $ORO_TOKEN..."
  $BINARY tx wasm execute "$INC_CONTRACT" '{
    "update_config": {
      "oro_token": { "native_token": { "denom": "'"$ORO_TOKEN"'" } },
      "vesting_contract": "'"$ADMIN_WALLET"'"
    }
  }' \
    --from "$KEY_NAME" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    -y
}

update_incentivization_fee() {
  echo "Updating incentivization fee to 1000000uzig..."
  $BINARY tx wasm execute "$INC_CONTRACT" '{
    "update_config": {
      "incentivization_fee_info": {
        "fee_receiver": "'"$ADMIN_WALLET"'",
        "fee": {
          "amount": "1000000",
          "denom": "uzig"
        }
      }
    }
  }' \
    --from "$KEY_NAME" \
    --chain-id "$CHAIN_ID" \
    --node "$RPC_URL" \
    --gas auto \
    --gas-adjustment "$GAS_ADJUSTMENT" \
    --fees "$FEES" \
    -y
}

show_config() {
  echo "Fetching incentives contract configuration..."
  echo "Contract: $INC_CONTRACT"
  echo "=================================="
  
  CONFIG=$(
    $BINARY query wasm contract-state smart "$INC_CONTRACT" '{"config":{}}' \
      --node "$RPC_URL" \
      --chain-id "$CHAIN_ID" \
      -o json
  )
  
  if [[ $? -eq 0 ]]; then
    echo "$CONFIG" | jq -r '.data | {
      "Owner": .owner,
      "Factory": .factory,
      "ORO Token": .oro_token,
      "ORO Per Second": .oro_per_second,
      "Total Alloc Points": .total_alloc_points,
      "Vesting Contract": .vesting_contract,
      "Generator Controller": .generator_controller,
      "Guardian": .guardian,
      "Incentivization Fee": .incentivization_fee_info,
      "Token Transfer Gas Limit": .token_transfer_gas_limit
    } | to_entries[] | "\(.key): \(.value)"'
  else
    echo "Error: Failed to fetch contract configuration"
    exit 1
  fi
}

# --- Dispatch command ---
COMMAND=${1:-help}; shift || true
case "$COMMAND" in
  instantiate-vesting)      instantiate_vesting ;;
  instantiate-incentives)   instantiate_incentives ;;
  setup-pools)             setup_pools "$@" ;;
  incentivize-external)    incentivize_external "$@" ;;
  stake-lp)                stake_lp "$@" ;;
  withdraw-lp)             withdraw_lp "$@" ;;
  claim-rewards)           claim_rewards "$@" ;;
  fetch-rewards)           fetch_rewards ;;
  update-config)           update_config ;;
  update-incentivization-fee) update_incentivization_fee ;;
  show-config)             show_config ;;
  *)                       usage ;;
esac
