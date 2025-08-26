#!/bin/bash
set -euo pipefail

# Source environment variables
source "$(dirname "$0")/../devnet.env"

# Configuration
OLD_CODE_ID=52
NEW_CODE_ID=55
CONTRACT_ADDR=${POOL_INITIALIZER_ADDR:-""}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Validate inputs


if [[ -z "$CONTRACT_ADDR" ]]; then
    print_error "POOL_INITIALIZER_ADDR is not set"
    echo "Please set POOL_INITIALIZER_ADDR in your environment or pass it as a parameter"
    exit 1
fi

print_status "Starting Pool Initializer Migration..."
print_status "Old Code ID: $OLD_CODE_ID"
print_status "New Code ID: $NEW_CODE_ID"
print_status "Contract Address: $CONTRACT_ADDR"
print_status "Chain ID: $CHAIN_ID"
echo ""

# Check if contract exists and get current info
print_status "Checking current contract information..."
CURRENT_INFO=$(zigchaind query wasm contract $CONTRACT_ADDR \
    --node $RPC_URL \
    --chain-id $CHAIN_ID \
    --output json 2>/dev/null || echo "")

if [[ -z "$CURRENT_INFO" ]]; then
    print_error "Contract not found at address: $CONTRACT_ADDR"
    exit 1
fi

CURRENT_CODE_ID=$(echo "$CURRENT_INFO" | jq -r '.contract_info.code_id')
print_success "Current Code ID: $CURRENT_CODE_ID"

if [[ "$CURRENT_CODE_ID" == "$NEW_CODE_ID" ]]; then
    print_warning "Contract is already running the new code ID"
    exit 0
fi

# Get contract admin
print_status "Getting contract admin..."
ADMIN_INFO=$(zigchaind query wasm contract $CONTRACT_ADDR \
    --node $RPC_URL \
    --chain-id $CHAIN_ID \
    --output json | jq -r '.contract_info.admin')

if [[ "$ADMIN_INFO" == "null" ]]; then
    print_error "Contract has no admin set. Cannot migrate."
    exit 1
fi

print_success "Contract Admin: $ADMIN_INFO"

# Check if the sender is the admin
SENDER_ADDR=$(zigchaind keys show $KEY_NAME -a --keyring-backend $KEYRING_BACKEND)
if [[ "$SENDER_ADDR" != "$ADMIN_INFO" ]]; then
    print_warning "Sender ($SENDER_ADDR) is not the contract admin ($ADMIN_INFO)"
    print_warning "Migration will fail if sender is not admin"
fi

# Confirm migration
echo ""
print_warning "About to migrate Pool Initializer contract:"
echo "  From Code ID: $CURRENT_CODE_ID"
echo "  To Code ID:   $NEW_CODE_ID"
echo "  Contract:     $CONTRACT_ADDR"
echo "  Admin:        $ADMIN_INFO"
echo "  Sender:       $SENDER_ADDR"
echo ""

read -p "Do you want to proceed with the migration? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    print_status "Migration cancelled"
    exit 0
fi

# Perform migration
print_status "Executing migration..."
MIGRATION_TX=$(zigchaind tx wasm migrate $CONTRACT_ADDR $NEW_CODE_ID '{}' \
    --from $KEY_NAME \
    --keyring-backend $KEYRING_BACKEND \
    --node $RPC_URL \
    --chain-id $CHAIN_ID \
    --gas auto \
    --gas-adjustment $GAS_ADJUSTMENT \
    --gas-prices $GAS_PRICES \
    --output json \
    -y)

if [[ $? -eq 0 ]]; then
    TX_HASH=$(echo "$MIGRATION_TX" | jq -r '.txhash')
    print_success "Migration transaction submitted!"
    print_status "Transaction Hash: $TX_HASH"
    
    # Wait for transaction to be processed
    print_status "Waiting for transaction to be processed..."
    sleep $SLEEP_TIME
    
    # Verify migration
    print_status "Verifying migration..."
    NEW_INFO=$(zigchaind query wasm contract $CONTRACT_ADDR \
        --node $RPC_URL \
        --chain-id $CHAIN_ID \
        --output json)
    
    NEW_CODE_ID_VERIFIED=$(echo "$NEW_INFO" | jq -r '.contract_info.code_id')
    
    if [[ "$NEW_CODE_ID_VERIFIED" == "$NEW_CODE_ID" ]]; then
        print_success "Migration successful!"
        print_status "Contract now running Code ID: $NEW_CODE_ID_VERIFIED"
        
        # Get contract version info
        CONTRACT_VERSION=$(zigchaind query wasm contract-state smart $CONTRACT_ADDR '{"config": {}}' \
            --node $RPC_URL \
            --chain-id $CHAIN_ID \
            --output json 2>/dev/null || echo "")
        
        if [[ -n "$CONTRACT_VERSION" ]]; then
            print_status "Contract is responding to queries"
        fi
        
    else
        print_error "Migration verification failed!"
        print_error "Expected Code ID: $NEW_CODE_ID"
        print_error "Actual Code ID: $NEW_CODE_ID_VERIFIED"
        exit 1
    fi
    
else
    print_error "Migration failed!"
    echo "$MIGRATION_TX"
    exit 1
fi

echo ""
print_success "Pool Initializer migration completed successfully!"
print_status "Contract Address: $CONTRACT_ADDR"
print_status "New Code ID: $NEW_CODE_ID"
print_status "Transaction Hash: $TX_HASH"
echo ""
print_status "Next steps:"
echo "  1. Test the migrated contract functionality"
echo "  2. Update your environment variables with the new code ID"
echo "  3. Monitor the contract for any issues"
