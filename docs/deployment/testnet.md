# Testnet Deployment Guide

This guide walks you through deploying Oroswap DEX contracts to the Zigchain testnet.

## 📋 Prerequisites

### Required Tools
```bash
# Install Zigchain CLI
curl -sSfL https://get.zigchain.com/install.sh | sh

# Install Rust and Cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install wasm-opt (optional, for optimization)
cargo install wasm-opt
```

### Environment Setup
```bash
# Set up Zigchain configuration
zigchaind config chain-id zig-test-2
zigchaind config keyring-backend test

# Create a test wallet
zigchaind keys add testnet-key --keyring-backend test

# Get testnet tokens from faucet
# Visit: https://faucet.zigchain.com/
```

## 🚀 Deployment Steps

### 1. Build Contracts
```bash
# Clone the repository
git clone https://github.com/oroswap/oroswap-core.git
cd oroswap-core

# Build optimized contracts
./scripts/build_release.sh

# Verify artifacts are created
ls -la opt-artifacts/
```

### 2. Store Contract Code
```bash
# Store all contracts
./scripts/store_code_testnet.sh

# Verify code storage
zigchaind query wasm list-code --node https://rpc.zigscan.net
```

### 3. Deploy Core Contracts

#### Deploy Coin Registry
```bash
cd scripts/tokens
./create_mint_token.sh

# Note the contract address for next steps
export COIN_REGISTRY_ADDR="zig1knyre4stvestyn032u9edf9w0fxhgv4szlwdvy2f69jludmunknswaxdsr"
```

#### Deploy Factory
```bash
cd ../factory
./factory_instantiate.sh

# Note the factory address
export FACTORY_CONTRACT="zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30"
```

#### Deploy Incentives
```bash
cd ../incentive
./instantiate_incentives.sh

# Note the incentives address
export INC_CONTRACT="zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7"
```

#### Deploy Router
```bash
cd ../router
./instantiate_router.sh

# Note the router address
export ROUTER_CONTRACT="zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a"
```

### 4. Configure Contracts

#### Update Factory Configuration
```bash
cd ../factory
./factory_queries.sh update_generator_address
```

#### Create Initial Pairs
```bash
# Create ZIG/USDC pair
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "create_pair": {
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "pair_type": {"xyk": {}}
  }
}' --from testnet-key --gas auto --fees 1000uzig --amount 1000000uzig

# Create ZIG/ORO pair
zigchaind tx wasm execute $FACTORY_CONTRACT '{
  "create_pair": {
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "uoro"}}
    ],
    "pair_type": {"xyk": {}}
  }
}' --from testnet-key --gas auto --fees 1000uzig --amount 1000000uzig
```

**Note**: Each pair creation requires a pool creation fee of 1,000,000 uzig (1 ZIG) and an LP token creation fee of 100,000,000 uzig (100 ZIG) sent via the `--amount` flag.

## 🔧 Configuration Files

### Environment Variables
Create `scripts/testnet.env`:
```bash
#!/bin/bash

# Network configuration
export CHAIN_ID="zig-test-2"
export RPC_URL="https://testnet-rpc.zigchain.com"
export KEY_NAME="testnet-key"
export KEYRING_BACKEND="test"

# Contract addresses (update with your deployed addresses)
export FACTORY_CONTRACT="zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30"
export ROUTER_CONTRACT="zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a"
export INC_CONTRACT="zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7"
export COIN_REGISTRY_ADDR="zig1knyre4stvestyn032u9edf9w0fxhgv4szlwdvy2f69jludmunknswaxdsr"

# Transaction settings
export SLEEP_TIME=5
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="1000uzig"
```

**Note**: For complete configuration details, see [Configuration](../configuration.md).

## 📊 Verification

### Test Basic Functionality
```bash
# Query factory config
zigchaind query wasm contract-state smart $FACTORY_CONTRACT '{"config": {}}' --node https://testnet-rpc.zigchain.com

# Query pairs
zigchaind query wasm contract-state smart $FACTORY_CONTRACT '{"pairs": {"limit": 10}}' --node https://testnet-rpc.zigchain.com

# Test swap simulation
zigchaind query wasm contract-state smart $ROUTER_CONTRACT '{
  "simulate_swap_operations": {
    "offer_amount": "1000000",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uzig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ]
  }
}' --node https://testnet-rpc.zigchain.com

## 🧪 Testing

### Create Test Tokens
```bash
# Create test USDC
zigchaind tx wasm execute $COIN_REGISTRY_ADDR '{
  "create_denom": {
    "subdenom": "usdc"
  }
}' --from testnet-key --gas auto --fees 1000uzig

# Mint test tokens
zigchaind tx wasm execute $COIN_REGISTRY_ADDR '{
  "mint_tokens": {
    "amount": "1000000000",
    "denom": "coin.zig1...usdc"
  }
}' --from testnet-key --gas auto --fees 1000uzig
```

### Test Swap
```bash
# Perform a test swap
zigchaind tx wasm execute $ROUTER_CONTRACT '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uzig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "950000",
    "max_spread": "0.01"
  }
}' --from testnet-key --gas auto --fees 1000uzig --amount 1000000uzig
```

## 🔍 Troubleshooting

### Common Issues

#### Gas Estimation Errors
```bash
# Use higher gas adjustment
export GAS_ADJUSTMENT="1.5"

# Or set specific gas
--gas 300000
```

#### Contract Not Found
```bash
# Verify contract exists
zigchaind query wasm contract $CONTRACT_ADDRESS --node https://testnet-rpc.zigchain.com

# Check if address is correct
zigchaind query wasm list-contract-by-code $CODE_ID --node https://testnet-rpc.zigchain.com
```

#### Insufficient Funds
```bash
# Check balance
zigchaind query bank balances $ADDRESS --node https://testnet-rpc.zigchain.com

# Get more testnet tokens
# Visit: https://faucet.zigchain.com/
```

#### Transaction Failures
```bash
# Check transaction status
zigchaind query tx $TX_HASH --node https://testnet-rpc.zigchain.com

# Check for specific errors
zigchaind query tx $TX_HASH --output json | jq '.tx_result.log'
```

## 📝 Deployment Checklist

- [ ] Build contracts successfully
- [ ] Store all contract code
- [ ] Deploy coin registry
- [ ] Deploy factory contract
- [ ] Deploy incentives contract
- [ ] Deploy router contract
- [ ] Configure factory with incentives address
- [ ] Create initial trading pairs
- [ ] Test basic swap functionality
- [ ] Verify all contracts on explorer
- [ ] Update frontend configuration

## 🔗 Useful Links

- [Zigchain Explorer](https://explorer.zigchain.com/)
- [Testnet Faucet](https://faucet.zigchain.com/)
- [RPC Endpoint](https://rpc.zigscan.net)
- [GitHub Repository](https://github.com/oroswap/oroswap-core)

## 📞 Support

If you encounter issues during deployment:
1. Check the [troubleshooting section](#troubleshooting)
2. Review [contract documentation](../contracts/)
3. Open an issue on [GitHub](https://github.com/oroswap/oroswap-core/issues)
4. Join our [Discord](https://discord.gg/oroswap) for community support