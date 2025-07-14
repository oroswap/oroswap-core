---
layout: default
title: Configuration
---

# Configuration

This document contains all the configuration parameters for the Oroswap testnet deployment.

## 🌐 Network Configuration

### Testnet Settings

```bash
# Chain configuration
export CHAIN_ID="zig-test-2"
export RPC_URL="https://testnet-rpc.zigchain.com"
export KEYRING_BACKEND="test"

# Transaction settings
export GAS_PRICES="0.25uzig"
export GAS_ADJUSTMENT="1.3"
export FEES="1000uzig"
```

### CLI Configuration

```bash
# Set up Zigchain CLI for testnet
zigchaind config chain-id zig-test-2
zigchaind config keyring-backend test
```

## 📋 Contract Addresses

### Testnet (v1.0.0)

```bash
# Core contracts
export FACTORY_CONTRACT="zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30"
export ROUTER_CONTRACT="zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a"
export INC_CONTRACT="zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7"
export COIN_REGISTRY_ADDR="zig1knyre4stvestyn032u9edf9w0fxhgv4szlwdvy2f69jludmunknswaxdsr"
```

## 🔧 Common Query Parameters

### Standard Query Format

```bash
# For all queries, use these parameters:
--node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Example Usage

```bash
# Query contract state
zigchaind query wasm contract-store <contract_address> '{"query": {}}' \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2

# Execute transaction
zigchaind tx wasm execute <contract_address> '{"execute": {}}' \
  --from <key_name> --gas auto --fees 1000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
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
- **RPC**: https://testnet-rpc.zigchain.com
- **GitHub**: https://github.com/oroswap/oroswap-core

## 📝 Environment Setup

### Complete Environment File

Create `scripts/testnet.env`:

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

### Usage

```bash
# Source the environment file
source scripts/testnet.env

# Now all variables are available
echo $FACTORY_CONTRACT
echo $CHAIN_ID
```
