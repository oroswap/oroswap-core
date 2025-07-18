---
layout: default
title: Smart Contracts Documentation
---

# Smart Contracts Documentation

Oroswap is built on a modular architecture with specialized contracts for different functions. Each contract has a specific role in the ecosystem.

## 🔗 Contract Documentation

### [Factory Contract](./factory.md)
The central hub for creating and managing trading pairs.

**Key Features:**
- Create new trading pairs
- Manage pair configurations
- Handle fee collection
- Coordinate with incentives

### [Pair Contracts](./pairs.md)
Multiple pair types for different trading strategies.

**Key Features:**
- XYK (Constant Product) pairs
- Stable pairs for stablecoins
- Concentrated liquidity pairs
- Configurable fee structures

### [Router Contract](./router.md)
Handles complex trading operations and multi-hop swaps.

**Key Features:**
- Multi-hop swaps
- Optimal route finding
- Slippage protection
- Batch operations

### [Incentives Contract](./incentives.md)
Manages liquidity mining rewards and staking mechanisms.

**Key Features:**
- LP token staking
- Reward distribution
- Pool management
- Time-based rewards

## 🏗️ Contract Architecture

The Oroswap ecosystem consists of several interconnected contracts:

### Core Contracts
- **Factory** - Creates and manages all pairs
- **Pairs** - Handle trading and liquidity provision
- **Router** - Manages complex swap operations
- **Incentives** - Distributes rewards to liquidity providers

### Supporting Contracts
- **Coin Registry** - Manages token metadata and precision
- **Token Factory Tracker** - Tracks token factory operations
- **Staking** - Additional staking mechanisms

## 🚀 Getting Started

To start using Oroswap contracts:

1. Read the [Factory Contract](./factory.md) documentation to understand pair creation
2. Learn about different [Pair Types](./pairs.md) and their use cases
3. Use the [Router Contract](./router.md) for complex trading operations
4. Explore [Incentives](./incentives.md) for earning rewards

## 📖 Contract Addresses

**Testnet (v1.0.0):**
- **Factory**: `zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30`
- **Router**: `zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a`
- **Incentives**: `zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7`
- **Coin Registry**: `zig1knyre4stvestyn032u9edf9w0fxhgv4szlwdvy2f69jludmunknswaxdsr`

## 🔗 Related Documentation

- **[Main Documentation](../index.md)** - Return to main documentation
- **[Transaction Examples](../transactions/)** - How to interact with contracts
- **[Events Documentation](../events/)** - Contract event monitoring
- **[Deployment Guide](../deployment/)** - How to deploy contracts 