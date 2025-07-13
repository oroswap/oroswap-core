# Pair Contracts

Oroswap supports multiple types of trading pairs, each optimized for different use cases and trading strategies.

> 📋 **Quick Reference**: See the [Transaction Index](../transactions.md#pair-contract) for all pair operations.

## 📋 Overview

**Supported Pair Types:**
- **XYK Pairs** - Constant Product AMM (like Uniswap V2)
- **Stable Pairs** - Optimized for stablecoin trading
- **Concentrated Pairs** - Concentrated liquidity (like Uniswap V3)

## 🔄 XYK Pairs (Constant Product)

### Overview
XYK (x * y = k) pairs use the constant product formula, where the product of reserves remains constant during swaps.

### Key Features
- **Simple AMM**: Uses the classic x * y = k formula
- **Slippage**: Higher slippage for large trades
- **Liquidity**: Uniform distribution across all price ranges
- **Fees**: Configurable swap fees (typically 0.3%)

### Create XYK Pair

```bash
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"xyk": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"token": {"contract_addr": "zig1..."}}
    ]
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

### Add Liquidity

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "assets": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"token": {"contract_addr": "zig1..."}}, "amount": "1000000"}
    ]
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

### Swap Tokens

```bash
zigchaind tx wasm execute <pair_address> '{
  "swap": {
    "offer_asset": {
      "info": {"native_token": {"denom": "uzig"}},
      "amount": "100000"
    },
    "belief_price": "1.0",
    "max_spread": "0.1"
  }
}' --from user --gas auto --fees 1000uzig --amount 100000uzig
```

## 🎯 Stable Pairs

### Overview
Stable pairs are optimized for trading between assets with similar values (like stablecoins), using a different mathematical formula to reduce slippage.

### Key Features
- **Low Slippage**: Optimized for similar-value assets
- **Amplification**: Uses amplification parameter for price stability
- **Stable Swaps**: Based on Curve's stable swap algorithm
- **Configurable**: Amplification can be adjusted

### Create Stable Pair

```bash
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"stable": {}},
    "asset_infos": [
      {"native_token": {"denom": "usdc"}},
      {"native_token": {"denom": "uoro"}}
    ],
    "init_params": "eyJhbXAiOjEwMH0="
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

**Init Params (Base64 encoded):**
```json
{
  "amp": 100
}
```

### Add Liquidity to Stable Pair

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "assets": [
      {"info": {"native_token": {"denom": "usdc"}}, "amount": "1000000"},
      {"info": {"native_token": {"denom": "uoro"}}, "amount": "1000000"}
    ]
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000usdc
```

## 🎯 Concentrated Pairs

### Overview
Concentrated liquidity pairs allow liquidity providers to concentrate their capital within specific price ranges, similar to Uniswap V3.

### Key Features
- **Concentrated Liquidity**: LPs can specify price ranges
- **Higher Capital Efficiency**: More liquidity in active price ranges
- **Multiple Fee Tiers**: Different fee levels for different ranges
- **Advanced Features**: Price oracles, TWAP, and more

### Create Concentrated Pair

```bash
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"concentrated": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"token": {"contract_addr": "zig1..."}}
    ],
    "init_params": "eyJwcmVjaXNpb24iOjE4LCJmZWVfcmF0ZSI6MTAwLCJhbXAiOjEwMCwiZ2FtbWEiOjEwLCJtaW5fdGlja190aWNrX3NwYWNpbmciOjEsIm1heF90aWNrX3RpY2tfc3BhY2luZyI6MTAwMDB9"
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

**Init Params (Base64 encoded):**
```json
{
  "precision": 18,
  "fee_rate": 100,
  "amp": 100,
  "gamma": 10,
  "min_tick_tick_spacing": 1,
  "max_tick_tick_spacing": 10000
}
```

### Add Concentrated Liquidity

```bash
zigchaind tx wasm execute <pair_address> '{
  "add_liquidity": {
    "lower_tick": 1000,
    "upper_tick": 2000,
    "amounts": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"token": {"contract_addr": "zig1..."}}, "amount": "1000000"}
    ],
    "max_spread": "0.1"
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

## 📊 Common Operations

### Query Pool Information

```bash
zigchaind query wasm contract-store <pair_address> '{"pool": {}}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Query Liquidity Provider Balance

```bash
zigchaind query wasm contract-store <pair_address> '{"balance": {"address": "zig1..."}}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Remove Liquidity

```bash
zigchaind tx wasm execute <pair_address> '{
  "withdraw_liquidity": {
    "amount": "1000000"
  }
}' --from user --gas auto --fees 1000uzig
```

### Collect Fees (Concentrated Pairs)

```bash
zigchaind tx wasm execute <pair_address> '{
  "collect_fees": {
    "lower_tick": 1000,
    "upper_tick": 2000
  }
}' --from user --gas auto --fees 1000uzig
```

## 🔧 Configuration

### Fee Structure
- **XYK Pairs**: Configurable swap fee (default ~0.3%)
- **Stable Pairs**: Lower fees for stable assets
- **Concentrated Pairs**: Multiple fee tiers (0.01%, 0.05%, 0.3%, 1%)

### Slippage Protection
All pairs support slippage protection to prevent front-running:

```json
{
  "max_spread": "0.1",  // 10% maximum slippage
  "belief_price": "1.0" // Expected price
}
```

### Minimum Liquidity
- **Minimum LP tokens**: 1000 units
- **Minimum trade size**: Configurable per pair type
- **Maximum spread**: Configurable protection

## 🔗 Related Examples

### Pair Creation
- **[Factory Contract](./factory.md#create-pair)** - Create pairs through factory
- **[Pool Creation Fee](./factory.md#create-pair)** - 1 ZIG fee for creating pairs

### Advanced Trading
- **[Multi-hop Swaps](./router.md#execute-swap-operations)** - Route through multiple pairs
- **[CW20 Token Swaps](./router.md#receive)** - Swap CW20 tokens via router
- **[Simulation](./router.md#simulate-swap-operations)** - Preview swap results

### Liquidity & Rewards
- **[Stake LP Tokens](./incentives.md#deposit)** - Earn rewards for providing liquidity
- **[Claim Rewards](./incentives.md#claim-rewards)** - Claim earned ORO tokens
- **[Pool Management](./incentives.md#setup-pools)** - Manage staking pools

### Transaction Index
- **[Complete Transaction Index](../transactions.md)** - All transaction examples in one place

## 🚨 Important Notes

1. **Pool Creation Fee**: 1 ZIG (1,000,000 uzig) required when creating pairs
2. **Liquidity Provision**: Must provide both assets in correct ratios
3. **Slippage Protection**: Always use appropriate slippage limits
4. **Fee Collection**: Concentrated pairs require manual fee collection
5. **Price Ranges**: Concentrated liquidity requires careful price range selection

## 📈 Performance Considerations

- **XYK**: Best for general trading, simple to understand
- **Stable**: Best for stablecoin pairs, lower slippage
- **Concentrated**: Best for capital efficiency, requires active management 