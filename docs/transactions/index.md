---
layout: default
title: Transaction Examples
---

# Transaction Examples

This section contains detailed examples for all transaction types in Oroswap DEX.

## 📋 Overview

These examples show how to interact with Oroswap contracts using the Zigchain CLI. Each example includes the complete command structure and explains the parameters.

## 🔗 Transaction Examples

### [Pair Creation](./pair-create.md)
Complete guide for creating different types of pairs:
- **XYK Pairs**: Constant product AMM pairs
- **Stable Pairs**: Optimized for stablecoin trading
- **Concentrated Pairs**: Advanced liquidity pools with custom parameters

**Key Features:**
- Base64 encoding for init parameters
- Pool creation fee (1 ZIG) and LP token creation fee (100 ZIG)
- Step-by-step instructions for each pair type

### [Swap Examples](./swap-examples.md)
Comprehensive swap transaction examples:
- **Router-based Swaps**: Multi-hop swaps with optimal routing
- **Direct Pair Swaps**: Single-hop swaps on specific pairs
- **CW20 Token Swaps**: Custom token trading
- **Concentrated Liquidity Swaps**: Advanced trading scenarios

**Key Features:**
- Slippage protection examples
- Fee calculation breakdown
- Simulation and reverse simulation
- Multi-hop routing examples

### [Liquidity Examples](./liquidity-examples.md)
Liquidity provision and removal examples:
- **Adding Liquidity**: To XYK, Stable, and Concentrated pairs
- **Removing Liquidity**: From all pair types
- **Auto-staking**: Automatic LP token staking
- **Query Operations**: Checking balances and pool information

**Key Features:**
- Slippage tolerance settings
- Gas optimization tips
- Balance querying examples

## 📊 Quick Reference

### Common Transaction Types
```bash
# Create pair
zigchaind tx wasm execute <factory_address> '{"create_pair": {...}}' --amount 101000000uzig

# Add liquidity
zigchaind tx wasm execute <pair_address> '{"provide_liquidity": {...}}' --amount <tokens>

# Swap tokens
zigchaind tx wasm execute <router_address> '{"execute_swap_operations": {...}}' --amount <tokens>

# Stake LP tokens
zigchaind tx wasm execute <incentives_address> '{"deposit": {...}}' --amount <lp_tokens>
```

### Fee Structure
- **Pool Creation**: 1,000,000 uzig (1 ZIG)
- **LP Token Creation**: 100,000,000 uzig (100 ZIG)
- **Swap Fee**: 0.1% for XYK pairs, configurable for others
- **Gas Fee**: Variable based on transaction complexity

## 🔧 Configuration

### Environment Setup
```bash
# Set up Zigchain CLI
zigchaind config chain-id <chain_id>
zigchaind config keyring-backend test

# Export contract addresses
# Note: For contract addresses, see the oroswap-deployments repository
export FACTORY_CONTRACT="<factory_address>"
export ROUTER_CONTRACT="<router_address>"
```

### Common Parameters
- **Gas**: `--gas auto` for automatic estimation
- **Fees**: `--fees 1000uzig` for transaction fees
- **Node**: `--node <rpc_url>`
- **Chain ID**: `--chain-id <chain_id>`

## 🚨 Important Notes

1. **Amount Format**: Use full denominations in `--amount` flag (e.g., `1000000uzig`)
2. **JSON Format**: Use raw numbers in JSON messages (e.g., `"1000000"`)
3. **Slippage Protection**: Always use `minimum_receive` or `max_spread`
4. **Gas Costs**: Complex operations require more gas
5. **Token Approval**: CW20 tokens need approval before use
6. **Address Verification**: Always verify contract addresses

## 🔗 Related Documentation

- **[Main Documentation](../index.md)** - Return to main documentation
- **[Contract Documentation](../contracts/)** - Contract-specific documentation
- **[Events Documentation](../events/)** - Event monitoring and parsing 