---
layout: default
title: Deployment Documentation
---
# Deployment Documentation

This section contains guides for deploying and configuring Oroswap DEX on different networks.

## 🔗 Deployment Guides

This section provides generic deployment guidance for Oroswap DEX contracts.

**Key Features:**

- Generic deployment instructions
- Contract instantiation guidance
- Environment configuration
- Testing and verification

## 🚀 Quick Start

1. **Setup Environment** - Configure Zigchain CLI and get network tokens
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
zigchaind config chain-id <chain_id>
zigchaind config keyring-backend test

# Create a test wallet
zigchaind keys add <key_name> --keyring-backend test

# Get network tokens from faucet
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

> **Note**: For contract addresses, see the [oroswap-deployments repository](https://github.com/oroswap/oroswap-deployments).

## 🔧 Configuration

### Environment Variables

```bash
#!/bin/bash

# Network configuration
export CHAIN_ID="<chain_id>"
export RPC_URL="<rpc_url>"
export KEY_NAME="<key_name>"
export KEYRING_BACKEND="test"

# Contract addresses
export FACTORY_CONTRACT="<factory_address>"
export ROUTER_CONTRACT="<router_address>"
export INC_CONTRACT="incentive_address"
export COIN_REGISTRY_ADDR="<coin_registry_address>"

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
- **Insufficient Funds** - Get more network tokens
- **Transaction Failures** - Check transaction logs for errors

## 🔗 Related Documentation

- **[Main Documentation](../index.md)** - Return to main documentation
- **[Contract Documentation](../contracts/)** - Contract-specific details
- **[Transaction Examples](../transactions/)** - How to interact with deployed contracts
- **[Events Documentation](../events/)** - Monitor contract events
