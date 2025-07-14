# Transaction Index

This document provides a quick reference to all main transactions available in the Oroswap protocol, organized by contract and functionality.

## Factory Contract

### Pool Management
- **[Create Pair](factory.md#create-pair)** - Create a new liquidity pool for two tokens
- **[Update Config](factory.md#update-config)** - Update factory configuration
- **[Update Pair Config](factory.md#update-pair-config)** - Update pair type configuration

### Queries
- **[Get Config](factory.md#get-factory-configuration)** - Query factory configuration
- **[Get Pairs](factory.md#list-all-pairs)** - List all pairs with pagination
- **[Get Pair](factory.md#get-pair-information)** - Query specific pair information

## Pair Contract

### Liquidity Management
- **[Provide Liquidity](pairs.md#provide-liquidity)** - Add liquidity to a pool
- **[Withdraw Liquidity](pairs.md#withdraw-liquidity)** - Remove liquidity from a pool

### Swapping
- **[Swap](pairs.md#swap)** - Execute a token swap
- **[Receive (CW20)](pairs.md#receive)** - Handle CW20 token swaps

### Queries
- **[Get Pool](pairs.md#query-pool-information)** - Query current pool state
- **[Get Balance](pairs.md#query-liquidity-provider-balance)** - Query LP token balance
- **[Simulation](pairs.md#simulate-swap)** - Preview swap results

## Router Contract

### Swapping
- **[Execute Swap Operations](router.md#execute-swap-operations)** - Multi-hop swaps with slippage protection
- **[Receive (CW20)](router.md#receive)** - Handle CW20 token swaps via router

### Queries
- **[Get Config](router.md#get-router-configuration)** - Query router configuration
- **[Simulate Swap Operations](router.md#simulate-swap-operations)** - Preview multi-hop swap results
- **[Reverse Simulate](router.md#reverse-simulation)** - Calculate input for desired output

## Incentives Contract

### Staking
- **[Deposit](incentives.md#deposit)** - Stake LP tokens to earn rewards
- **[Withdraw](incentives.md#withdraw)** - Unstake LP tokens
- **[Claim Rewards](incentives.md#claim-rewards)** - Claim earned rewards

### Pool Management (Admin)
- **[Setup Pools](incentives.md#setup-pools)** - Configure staking pools
- **[Incentivize](incentives.md#incentivize)** - Add reward schedules to pools

### Queries
- **[Get Config](incentives.md#get-configuration)** - Query contract configuration
- **[Get Deposit](incentives.md#get-deposit)** - Query user staked amount
- **[Get Pending Rewards](incentives.md#get-pending-rewards)** - Query pending rewards
- **[Get Pool Info](incentives.md#get-pool-information)** - Query pool information

## Quick Reference by Functionality

### Trading
- [Execute Swap Operations](router.md#execute-swap-operations) (Router) - Multi-hop swaps
- [Swap](pairs.md#swap) (Pair) - Direct pair swap
- [Simulate Swap Operations](router.md#simulate-swap-operations) (Router) - Preview swaps

### Liquidity
- [Provide Liquidity](pairs.md#provide-liquidity) (Pair) - Add liquidity to pairs
- [Withdraw Liquidity](pairs.md#withdraw-liquidity) (Pair) - Remove liquidity from pairs

### Staking & Rewards
- [Deposit](incentives.md#deposit) (Incentives) - Stake LP tokens
- [Claim Rewards](incentives.md#claim-rewards) (Incentives) - Claim earned rewards

### Pool Management
- [Create Pair](factory.md#create-pair) (Factory) - Create new pool
- [Get Pair](factory.md#get-pair-information) (Factory) - Query pool info

---

For detailed examples and configuration options, refer to the individual contract documentation pages. 