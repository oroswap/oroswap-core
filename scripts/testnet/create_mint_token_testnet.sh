#!/bin/bash
# Script to create a single factory denom, mint tokens, and check balance on testnet.

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────
BINARY="zigchaind"
RPC_URL="https://testnet-rpc.zigchain.com"
CHAIN_ID="zig-test-2"
KEY_NAME="testnet-admin"
KEYRING_BACKEND="file"
MAX_SUPPLY="1000000000000000"
CAN_CHANGE_SUPPLY="true"
MINT_AMOUNT="50000000000000"
BLOCK_TIME=3

# Sub-denom and metadata (hardcoded)
SUB_DENOM="uoro"
METADATA_URI="https://blue-careful-carp-364.mypinata.cloud/ipfs/bafkreifhxwnh36zccf7nl7yb2tv57gcpcqjtrpeidhzwbeyxz23skvdnv4"

# ─── Derived values ─────────────────────────────────────────────────────────
DEFAULT_WALLET_ADDRESS=$($BINARY keys show "$KEY_NAME" -a --keyring-backend "$KEYRING_BACKEND")
URI_HASH=$(echo -n "$METADATA_URI" | shasum -a 256 | awk '{print $1}')
# New denom ID uses 'coin.' prefix as seen in on-chain events
FULL_DENOM_ID="coin.$DEFAULT_WALLET_ADDRESS.$SUB_DENOM"

# ─── Create denom ────────────────────────────────────────────────────────────
echo "🔨 Creating factory denom '$SUB_DENOM' with metadata URI '$METADATA_URI' on testnet..."
$BINARY tx factory create-denom "$SUB_DENOM" "$MAX_SUPPLY" "$CAN_CHANGE_SUPPLY" \
  "$METADATA_URI" "$URI_HASH" \
  --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  --gas-prices 0.5uzig --gas auto --gas-adjustment 1.3 -y -o json | jq

sleep "$BLOCK_TIME"

# ─── Mint and send tokens ────────────────────────────────────────────────────
echo "📬 Minting $MINT_AMOUNT of '$FULL_DENOM_ID' to wallet $DEFAULT_WALLET_ADDRESS on testnet..."
$BINARY tx factory mint-and-send-tokens "${MINT_AMOUNT}${FULL_DENOM_ID}" \
  "$DEFAULT_WALLET_ADDRESS" \
  --from "$KEY_NAME" --keyring-backend "$KEYRING_BACKEND" \
  --node "$RPC_URL" --chain-id "$CHAIN_ID" \
  --gas-prices 0.25uzig --gas auto --gas-adjustment 1.3 -y -o json | jq

sleep "$BLOCK_TIME"

# ─── Check balance ───────────────────────────────────────────────────────────
echo "🔍 Checking balance for '$FULL_DENOM_ID' on testnet..."
$BINARY query bank balances "$DEFAULT
