#!/bin/bash
set -euo pipefail
source ../../devnet_testing.env

ADDR=$($BINARY keys show $KEY_NAME -a --keyring-backend $KEYRING_BACKEND)
TOKEN="coin.zig12jzwc0a3pyv4dze0t252qkwf77t4vs5rqfn3zc.oro"
NOW=$(date +%s)
V_START=$NOW
V_END=$(($NOW + 3600))
EXPIRY=$(($NOW + 7200))   # 2 h after V_END — mandatory and must be > vesting_end_time

echo "Creating airdrop..."
./test.sh add_airdrop "$TOKEN" "$NOW" "$V_START" "$V_END" "100" "0" "[]" "$EXPIRY" > /dev/null
sleep 6

LATEST=$(./test.sh query_latest_id | grep -o '"id": [0-9]*' | awk '{print $2}')
echo "Airdrop ID: $LATEST"

echo "Adding eligibility for $ADDR..."
./test.sh update_eligibility $LATEST "[[\"$ADDR\", \"1000\"]]" > /dev/null
sleep 6

echo "--- ATTEMPTING CLAIM WITHOUT FUNDS ---"
./test.sh claim $LATEST
