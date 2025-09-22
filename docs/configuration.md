---
layout: default
title: Configuration
---

# Configuration

This document contains all the configuration parameters for Oroswap DEX deployment.

## 🌐 Network Configuration

### Network Settings

```bash
# Chain configuration
export CHAIN_ID="<chain_id>"
export RPC_URL="<rpc_url>"
export KEYRING_BACKEND="test"

# Transaction settings
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="1000uzig"
```

### CLI Configuration

```bash
# Set up Zigchain CLI
zigchaind config chain-id <chain_id>
zigchaind config keyring-backend test
```

## 📋 Contract Addresses

> **Note**: Contract addresses are maintained separately in the [oroswap-deployments repository](https://github.com/oroswap/oroswap-deployments). Please refer to that repository for the latest contract addresses for your specific network.

## 🔧 Common Query Parameters

### Standard Query Format

```bash
# For all queries, use these parameters:
--node <rpc_url> --chain-id <chain_id>
```

### Example Usage

```bash
# Query contract state
zigchaind query wasm contract-state smart <contract_address> '{"query": {}}' \
  --node <rpc_url> --chain-id <chain_id>

# Execute transaction
zigchaind tx wasm execute <contract_address> '{"execute": {}}' \
  --from <key_name> --gas auto --fees 1000uzig \
  --node <rpc_url> --chain-id <chain_id>
```

## 🎯 Token Configuration

### Supported Tokens

- **ZIG**: `uzig` (native token)
- **ORO**: `uoro` (native token)

### Token Examples

```bash
# Native token examples
{"native_token": {"denom": "uzig"}}
{"native_token": {"denom": "uoro"}}
{"native_token": {"denom": "usdc"}}

# CW20 token example
{"token": {"contract_addr": "zig1..."}}
```

## 🔗 Useful Links

- **ZIG Faucet**: https://faucet.zigchain.com/
- **RPC**: <rpc_url>
- **GitHub**: https://github.com/oroswap/oroswap-core

## 📝 Environment Setup

### Complete Environment File

Create `scripts/network.env`:

```bash
#!/bin/bash

# Network configuration
export CHAIN_ID="<chain_id>"
export RPC_URL="<rpc_url>"
export KEY_NAME="<key_name>"
export KEYRING_BACKEND="test"

# Contract addresses (get from oroswap-deployments repository)
export FACTORY_CONTRACT="<factory_address>"
export ROUTER_CONTRACT="<router_address>"
export INC_CONTRACT="<incentives_address>"
export COIN_REGISTRY_ADDR="<coin_registry_address>"

# Transaction settings
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="1000uzig"
```

### Usage

```bash
# Source the environment file
source scripts/network.env

# Now all variables are available
echo $FACTORY_CONTRACT
echo $CHAIN_ID
```
