---
layout: default
title: Deployment Documentation
---

# Deployment Documentation

This section contains guides for deploying and configuring Oroswap DEX on different networks.

## 🔗 Deployment Guides

### [Testnet Deployment](./testnet.md)
Complete guide for deploying Oroswap contracts to the Zigchain testnet.

**Key Features:**
- Step-by-step deployment instructions
- Contract instantiation scripts
- Environment configuration
- Testing and verification

## 🚀 Quick Start

1. **Setup Environment** - Configure Zigchain CLI and get testnet tokens
2. **Build Contracts** - Compile optimized contract artifacts
3. **Deploy Contracts** - Instantiate all core contracts
4. **Configure Contracts** - Set up factory and incentives addresses
5. **Create Initial Pairs** - Set up basic trading pairs
6. **Test Functionality** - Verify all operations work correctly

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

## 🏗️ Deployment Architecture

### Contract Deployment Order
1. **Coin Registry** - Token metadata management
2. **Factory** - Pair creation and management
3. **Incentives** - Reward distribution system
4. **Router** - Multi-hop swap routing

### Configuration Dependencies
- Factory needs incentives address
- Router needs factory address
- All contracts need coin registry address

## 📊 Contract Addresses

**Testnet (v1.0.0):**
- **Factory**: `zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30`
- **Router**: `zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a`
- **Incentives**: `zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7`
- **Coin Registry**: `zig1knyre4stvestyn032u9edf9w0fxhgv4szlwdvy2f69jludmunknswaxdsr`

## 🔧 Configuration

### Environment Variables
```bash
#!/bin/bash

# Network configuration
export CHAIN_ID="zig-test-2"
export RPC_URL="https://testnet-rpc.zigchain.com"
export KEY_NAME="testnet-key"
export KEYRING_BACKEND="test"

# Contract addresses
export FACTORY_CONTRACT="zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30"
export ROUTER_CONTRACT="zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a"
export INC_CONTRACT="zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7"
export COIN_REGISTRY_ADDR="zig1knyre4stvestyn032u9edf9w0fxhgv4szlwdvy2f69jludmunknswaxdsr"

# Transaction settings
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="1000uzig"
```

## 🧪 Testing

### Verification Steps
1. **Query Factory Config** - Verify factory is properly configured
2. **List Pairs** - Check if pairs are created correctly
3. **Test Swap Simulation** - Verify router can simulate swaps
4. **Check Contract State** - Ensure all contracts are active

### Common Issues
- **Gas Estimation Errors** - Use higher gas adjustment
- **Contract Not Found** - Verify contract addresses
- **Insufficient Funds** - Get more testnet tokens
- **Transaction Failures** - Check transaction logs for errors

## 🔗 Related Documentation

- **[Main Documentation](../index.md)** - Return to main documentation
- **[Contract Documentation](../contracts/)** - Contract-specific details
- **[Transaction Examples](../transactions/)** - How to interact with deployed contracts
- **[Events Documentation](../events/)** - Monitor contract events 