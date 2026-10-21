---
layout: default
title: Oroswap Documentation
---

# Oroswap Documentation

Welcome to the Oroswap DEX documentation! This guide will help you understand, deploy, and interact with the Oroswap decentralized exchange on Zigchain.

## 📚 Documentation Sections

### [Contracts](./contracts/)

- [Factory Contract](./contracts/factory.md) - Central pair creation and management
- [Pair Contracts](./contracts/pairs.md) - XYK, Stable, and Concentrated liquidity pairs
- [Router Contract](./contracts/router.md) - Multi-hop swaps and routing
- [Incentives Contract](./contracts/incentives.md) - Staking and reward distribution

### [Events](./events/)

- [Factory Events](./events/factory-events.md) - Events emitted by factory contract
- [Pair Events](./events/pair-events.md) - Events emitted by pair contracts
- [Router Events](./events/router-events.md) - Events emitted by router contract

### [Transactions](./transactions/)

- **[Transaction Index](./transactions/)** - Complete reference of all transactions
- [Swap Examples](./transactions/swap-examples.md) - How to perform token swaps
- [Liquidity Examples](./transactions/liquidity-examples.md) - Adding/removing liquidity
- [Pair Creation](./transactions/pair-create.md) - Creating XYK, Stable, and Concentrated pairs

### [Deployment](./deployment/)

- [Configuration](./configuration.md) - Network and contract configuration

## 🚀 Quick Start

1. **Setup Environment** - [Deployment Guide](./deployment/)
2. **Deploy Contracts** - [Deployment Guide](./deployment/)
3. **Create Pairs** - [Pair Creation](./transactions/pair-create.md) or [Factory Contract](./contracts/factory.md)
4. **Start Trading** - [Transaction Index](./transactions/) or [Swap Examples](./transactions/swap-examples.md)

## 🔗 Useful Links

- [GitHub Repository](https://github.com/oroswap/oroswap-core)
- [Frontend](https://oroswap.org/)
- [Zigchain Explorer](https://explorer.zigchain.com/)

## 📖 Contract Addresses

> **Note**: For contract addresses, see the [oroswap-deployments repository](https://github.com/oroswap/oroswap-deployments).

## 🏗️ Architecture Overview

Oroswap is built on a modular architecture with specialized contracts:

### Core Contracts

- **Factory**: Creates and manages all trading pairs
- **Pairs**: Handle trading and liquidity provision (XYK, Stable, Concentrated)
- **Router**: Manages complex swap operations and multi-hop routing
- **Incentives**: Distributes rewards to liquidity providers

### Supporting Contracts

- **Coin Registry**: Manages token metadata and precision
- **Token Factory Tracker**: Tracks token factory operations
- **Staking**: ORO staking with multi-denom rewards and an unbonding period
- **Airdrop**: Token airdrops with claim, linear vesting and APY bonus
- **Wrapper**: 1:1 lock-and-mint wrapper for factory tokens
- **Revenue Router**: Bridges USDC from ZIG Chain to Ethereum via Noble CCTP

## 🎯 Key Features

### Multiple Pair Types

- **XYK Pairs**: Constant product AMM (like Uniswap V2)
- **Stable Pairs**: Optimized for stablecoin trading
- **Concentrated Pairs**: Advanced liquidity pools with custom parameters

### Advanced Trading

- **Multi-hop Swaps**: Route through multiple pairs
- **Slippage Protection**: Built-in protection against front-running
- **Optimal Routing**: Automatic route optimization

### Liquidity Mining

- **LP Token Staking**: Stake LP tokens to earn rewards
- **Configurable Rewards**: Adjustable reward rates per pool
- **Time-based Distribution**: Rewards distributed over time periods

## 📊 Monitoring

### Key Metrics

- **Total Value Locked (TVL)**: Total liquidity in all pairs
- **Trading Volume**: Daily/weekly trading volume
- **Active Pairs**: Number of active trading pairs
- **Liquidity Providers**: Number of active LPs

### Query Examples

```bash
# Get factory configuration
zigchaind query wasm contract-store <factory_address> '{"config": {}}'

# List all pairs
zigchaind query wasm contract-store <factory_address> '{"pairs": {}}'

# Get pool information
zigchaind query wasm contract-store <pair_address> '{"pool": {}}'
```
## 🚨 Security

### Best Practices

1. **Always verify contract addresses** before interacting
2. **Use appropriate slippage limits** to prevent front-running
3. **Test with small amounts** before large transactions
4. **Monitor gas costs** for complex operations
5. **Keep private keys secure** and never share them

### Audit Status

- Factory Contract: ✅ Audited
- Pair Contracts: ✅ Audited
- Router Contract: ✅ Audited
- Incentives Contract: ✅ Audited

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](https://github.com/oroswap/oroswap-core/blob/main/CONTRIBUTING.md) for details.

### Development Setup

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📞 Support

- **Documentation Issues**: Open an issue on GitHub
- **Technical Questions**: Join our Socials
- **Feature Requests**: Submit via GitHub discussions

## 📄 License

This project is licensed under the GPL-3.0 License - see the [LICENSE](https://github.com/oroswap/oroswap-core/blob/main/LICENSE) file for details.
