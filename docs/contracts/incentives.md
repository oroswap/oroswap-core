# Incentives Contract

The Incentives contract manages liquidity mining rewards, staking mechanisms, and token distribution for the Oroswap ecosystem.

> 📋 **Quick Reference**: See the [Transaction Index](../transactions.md#incentives-contract) for all incentives operations.

## 📋 Overview

**Contract Address**: `zig1sq7mu45and7htxdjwe9htl0q3y33qlnt6cded6z299303pya5d0qda8sg7` (Testnet)

**Purpose**:

- Distribute rewards for liquidity provision (rewards can be in ORO or other tokens)
- Manage staking pools and reward rates
- Handle reward claiming and distribution
- Coordinate with factory and pair contracts

## 🏗️ Core Components

### Staking Pools

- **LP Token Staking**: Stake LP tokens to earn rewards
- **Configurable Rewards**: Adjustable reward rates per pool
- **Multiple Pools**: Support for different pair types

### Reward Distribution

- **Time-based Rewards**: Rewards distributed over time periods
- **Proportional Distribution**: Rewards based on staked amount and time
- **Multiple Reward Tokens**: Pools can distribute more than one reward token (e.g., ORO, ZIG, or others)

## 🔄 Staking Operations

### Deposit LP Tokens

```bash
zigchaind tx wasm execute <incentives_address> '{
  "deposit": {
    "recipient": "zig1..."
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000coin.zig1..lptoken
```

**For CW20 LP tokens:**

```bash
zigchaind tx wasm execute <lp_token_address> '{
  "send": {
    "contract": "<incentives_address>",
    "amount": "1000000",
    "msg": "eyJkZXBvc2l0Ijp7InJlY2lwaWVudCI6InppZzEuLi4ifX0="
  }
}' --from user --gas auto --fees 1000uzig
```

### Withdraw LP Tokens

```bash
zigchaind tx wasm execute <incentives_address> '{
  "withdraw": {
    "lp_token": "coin.zig1..lptoken",
    "amount": "500000"
  }
}' --from user --gas auto --fees 1000uzig
```

### Claim Rewards

```bash
zigchaind tx wasm execute <incentives_address> '{
  "claim_rewards": {
    "lp_tokens": ["coin.zig1..lptoken"]
  }
}' --from user --gas auto --fees 1000uzig
```

## 🎯 Pool Management (Admin Only)

### Setup Pools

```bash
zigchaind tx wasm execute <incentives_address> '{
  "setup_pools": {
    "pools": [
      ["coin.zig1..lptoken", "100"],
      ["coin.zig1..lptoken", "200"]
    ]
  }
}' --from admin --gas auto --fees 1000uzig
```

### Incentivize Pool

```bash
zigchaind tx wasm execute <incentives_address> '{
  "incentivize": {
    "lp_token": "coin.zig1..lptoken",
    "schedule": {
      "reward": {
        "info": {"native_token": {"denom": "uzig"}},
        "amount": "1000000"
      },
      "duration_periods": 1
    }
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

## �� Query Operations

### Get Configuration

```bash
zigchaind query wasm contract-store <incentives_address> '{
  "config": {}
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get User Deposit

```bash
zigchaind query wasm contract-store <incentives_address> '{
  "deposit": {
    "lp_token": "coin.zig1..lptoken",
    "user": "zig1..."
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Pending Rewards

```bash
zigchaind query wasm contract-store <incentives_address> '{
  "pending_rewards": {
    "lp_token": "coin.zig1..lptoken",
    "user": "zig1..."
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Pool Information

```bash
zigchaind query wasm contract-store <incentives_address> '{
  "pool_info": {
    "lp_token": "coin.zig1..lptoken"
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Reward Info

```bash
zigchaind query wasm contract-store <incentives_address> '{
  "reward_info": {
    "lp_token": "coin.zig1..lptoken"
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 🔗 Related Examples

### Liquidity Provision

- **[Add Liquidity](./pairs.md#provide-liquidity)** - Provide liquidity to earn LP tokens
- **[Remove Liquidity](./pairs.md#withdraw-liquidity)** - Withdraw liquidity from pairs
- **[Concentrated Pairs](./pairs.md#concentrated-pairs)** - Add liquidity to concentrated pairs

### Trading Operations

- **[Swap Tokens](./pairs.md#swap)** - Trade tokens on pairs
- **[Multi-hop Swaps](./router.md#execute-swap-operations)** - Complex trading routes
- **[CW20 Token Swaps](./router.md#receive)** - Swap CW20 tokens

### Pair Management

- **[Create Pairs](./factory.md#create-pair)** - Create new trading pairs
- **[Factory Configuration](./factory.md#update-config)** - Update factory settings

### Transaction Index

- **[Complete Transaction Index](../transactions.md)** - All transaction examples in one place

## 🚨 Important Considerations

### Staking Requirements

1. **LP Token Ownership**: Must own LP tokens to stake
2. **Pool Activation**: Pool must be active to accept stakes
3. **Minimum Stakes**: Some pools may have minimum stake requirements

### Reward Distribution

1. **Time-based**: Rewards accumulate over time
2. **Proportional**: Based on staked amount and time
3. **Claimable**: Must manually claim rewards

## 📈 Best Practices

### For Users

1. **Stake Early**: Earlier stakers get more rewards
2. **Monitor Pools**: Check pool status and reward rates
3. **Claim Regularly**: Don't let rewards accumulate too much
4. **Diversify**: Stake in multiple pools for better returns

### For Admins

1. **Fair Distribution**: Set allocation points fairly
2. **Monitor Activity**: Track pool performance
3. **Adjust Rates**: Update reward rates based on activity
4. **Plan Ahead**: Set appropriate time windows

## 🔧 Integration with Other Contracts

### Factory Integration

- Factory contract references incentives contract
- New pairs can be automatically added to incentives
- Pool creation triggers incentive pool setup

### Pair Integration

- LP tokens are automatically stakable
- Auto-staking options for liquidity providers
- Reward distribution coordination

## 🚨 Error Handling

Common errors and solutions:

- **Insufficient Balance**: Ensure you have enough LP tokens
- **Pool Not Active**: Check if pool is accepting stakes
- **No Rewards**: Verify reward rates and staking duration
- **Admin Only**: Pool management requires admin privileges
- **Invalid Time**: Check pool start/end times

## 📊 Monitoring and Analytics

### Key Metrics to Track

- **Total Value Locked (TVL)**: Total staked LP tokens
- **Reward Distribution**: ORO tokens distributed per day
- **Pool Performance**: Individual pool statistics
- **User Participation**: Number of active stakers

### Query Examples

```bash
# Get total staked amount
zigchaind query wasm contract-store <incentives_address> '{"total_staked": {}}'

# Get reward rate
zigchaind query wasm contract-store <incentives_address> '{"oro_per_second": {}}'

# Get user count
zigchaind query wasm contract-store <incentives_address> '{"user_count": {}}'
```
