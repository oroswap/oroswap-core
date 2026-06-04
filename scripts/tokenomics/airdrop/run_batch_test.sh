#!/bin/bash
set -euo pipefail

source "$(dirname "$0")/../../devnet_testing.env"

TOKEN="coin.zig12jzwc0a3pyv4dze0t252qkwf77t4vs5rqfn3zc.oro"
START=$(($(date +%s) + 45))
V_START=$(($(date +%s) + 345))
V_END=$(($(date +%s) + 7200))
EXPIRY=$(($(date +%s) + 14400))   # 4 h — must be > V_END
INITIAL_CLAIM=40
APY=100

echo "1) Creating empty airdrop..."
./test.sh add_airdrop "$TOKEN" "$START" "$V_START" "$V_END" "$INITIAL_CLAIM" "$APY" "[]" "$EXPIRY" > /dev/null
sleep 6

echo "Getting Latest ID..."
LATEST=$(./test.sh query_latest_id | grep -o '"id": [0-9]*' | awk '{print $2}')
echo "Created Airdrop ID: $LATEST"

echo "2) Adding batch 1 (2 addresses)..."
./test.sh update_eligibility $LATEST '[["zig1zatk35sgekrgshl4rz66cc2e7vafd80672yq90","5338867"],["zig1dqn4mnaaj9qwh60sr0t3gszlshvmtu4epum00f","4300500"]]' > /dev/null
sleep 6

echo "3) Adding batch 2 (3 addresses)..."
./test.sh update_eligibility $LATEST '[["zig1mljqn7hmezdapez3py8s6zn3nm53etpv29yce2","2335430"],["zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl","1000"],["zig12dxqlvccndj447q86sax67senmy65csnmvvjur","2000"]]' > /dev/null
sleep 6

echo "4) Querying status for 5 addresses in airdrop $LATEST..."
./test.sh query_user_airdrop_status $LATEST "zig1zatk35sgekrgshl4rz66cc2e7vafd80672yq90"
./test.sh query_user_airdrop_status $LATEST "zig1dqn4mnaaj9qwh60sr0t3gszlshvmtu4epum00f"
./test.sh query_user_airdrop_status $LATEST "zig1mljqn7hmezdapez3py8s6zn3nm53etpv29yce2"
./test.sh query_user_airdrop_status $LATEST "zig12zs35pj7hymnkmu7njg5q4gmp8kmp36pjc34jl"
./test.sh query_user_airdrop_status $LATEST "zig12dxqlvccndj447q86sax67senmy65csnmvvjur"

echo "Done!"
