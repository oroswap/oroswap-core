#!/bin/bash

# Load environment variables
source scripts/devnet.env

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🔄 Updating Pool Initializer Configuration${NC}"
echo ""

# Check if POOL_INITIALIZER_ADDR is set
if [ -z "$POOL_INITIALIZER_ADDR" ]; then
    echo -e "${RED}❌ POOL_INITIALIZER_ADDR not set in devnet.env${NC}"
    exit 1
fi

# Check if KEY_NAME is set
if [ -z "$KEY_NAME" ]; then
    echo -e "${RED}❌ KEY_NAME not set in devnet.env${NC}"
    exit 1
fi

echo -e "${GREEN}📋 Current Configuration:${NC}"
zigchaind query wasm contract-state smart $POOL_INITIALIZER_ADDR '{"config": {}}' --output json | jq '.data'

echo ""
echo -e "${YELLOW}🔧 Update Options:${NC}"
echo "1. Update pair creation fee only"
echo "2. Update factory address only"
echo "3. Update both"
echo "4. Exit"

read -p "Choose an option (1-4): " choice

case $choice in
    1)
        read -p "Enter new pair creation fee (in uzig, e.g., 101000000 for 101 ZIG): " new_fee
        echo -e "${GREEN}📝 Updating pair creation fee to ${new_fee} uzig...${NC}"
        
        zigchaind tx wasm execute $POOL_INITIALIZER_ADDR \
            "{\"update_config\": {\"pair_creation_fee\": \"$new_fee\"}}" \
            --from $KEY_NAME --keyring-backend $KEYRING_BACKEND \
            --chain-id $CHAIN_ID --gas auto --gas-adjustment 1.3 \
            --yes --output json | jq '.txhash'
        ;;
    2)
        read -p "Enter new factory address: " new_factory
        echo -e "${GREEN}📝 Updating factory address to ${new_factory}...${NC}"
        
        zigchaind tx wasm execute $POOL_INITIALIZER_ADDR \
            "{\"update_config\": {\"factory_addr\": \"$new_factory\"}}" \
            --from $KEY_NAME --keyring-backend $KEYRING_BACKEND \
            --chain-id $CHAIN_ID --gas auto --gas-adjustment 1.3 \
            --yes --output json | jq '.txhash'
        ;;
    3)
        read -p "Enter new pair creation fee (in uzig, e.g., 101000000 for 101 ZIG): " new_fee
        read -p "Enter new factory address: " new_factory
        echo -e "${GREEN}📝 Updating both pair creation fee and factory address...${NC}"
        
        zigchaind tx wasm execute $POOL_INITIALIZER_ADDR \
            "{\"update_config\": {\"pair_creation_fee\": \"$new_fee\", \"factory_addr\": \"$new_factory\"}}" \
            --from $KEY_NAME --keyring-backend $KEYRING_BACKEND \
            --chain-id $CHAIN_ID --gas auto --gas-adjustment 1.3 \
            --yes --output json | jq '.txhash'
        ;;
    4)
        echo -e "${GREEN}👋 Exiting...${NC}"
        exit 0
        ;;
    *)
        echo -e "${RED}❌ Invalid option${NC}"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}✅ Configuration updated successfully!${NC}"
echo ""
echo -e "${YELLOW}📋 New Configuration:${NC}"
sleep 3
zigchaind query wasm contract-state smart $POOL_INITIALIZER_ADDR '{"config": {}}' --output json | jq '.data'
