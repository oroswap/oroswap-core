# Pair Contracts

This document describes the different types of pair contracts available in Oroswap and how to interact with them.

> **Note**: For contract addresses, see the [oroswap-deployments repository](https://github.com/oroswap/oroswap-deployments).

## 📋 Overview

Oroswap supports three types of pair contracts:

- **XYK Pairs**: Constant product AMM
- **Stable Pairs**: Optimized for stablecoin trading
- **Concentrated Pairs**: Advanced liquidity pools with custom parameters (not Uniswap V3)

## 🎯 XYK Pairs

### Overview

XYK (x * y = k) pairs use the constant product formula, making them suitable for most trading pairs.

### Key Features

- **Simple Formula**: x * y = k
- **High Liquidity**: Good for general trading
- **Predictable**: Easy to understand price impact
- **Configurable**: Adjustable swap fees (default 0.1%)

### Create XYK Pair

#### Basic XYK Pair (0.1% fee)
```bash
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"xyk": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "init_params": null
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig
```

#### Custom XYK Pair Types

Oroswap supports custom XYK pair types with different fee structures:

- **xyk_10**: 0.1% total fee (10 bps)
- **xyk_25**: 0.25% total fee (25 bps)  
- **xyk_100**: 1% total fee (100 bps)
- **xyk_200**: 2% total fee (200 bps)

```bash
# Create XYK pair with 1% fee
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"custom": "xyk_100"},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "init_params": null
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig
```

```bash
# Create XYK pair with 0.25% fee
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"custom": "xyk_25"},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "init_params": null
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig
```

### Add Liquidity to XYK Pair

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "assets": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"token": {"contract_addr": "zig1..."}}, "amount": "1000000"}
    ],
    "slippage_tolerance": "0.01"
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000uzig
```

### Swap on XYK Pair

```bash
zigchaind tx wasm execute <pair_address> '{
  "swap": {
    "offer_asset": {
      "info": {"native_token": {"denom": "uzig"}},
      "amount": "1000000"
    },
    "belief_price": "1.0",
    "max_spread": "0.1"
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

## 🎯 Stable Pairs

### Overview

Stable pairs are optimized for trading between assets with similar values (like stablecoins), using a different mathematical formula to reduce slippage.

### Key Features

- **Low Slippage**: Optimized for similar-value assets
- **Amplification**: Uses amplification parameter for price stability
- **Stable Swaps**: Based on Curve's stable swap algorithm
- **Configurable**: Amplification can be adjusted
- **Lower Fees**: Default swap fee is 0.04%

### Create Stable Pair

**Important**: Before creating stable pairs, you must first add the stable pair type to the factory configuration.

#### Step 1: Add Stable Pair Type to Factory

```bash
zigchaind tx wasm execute <factory_address> '{
  "update_pair_config": {
    "pair_type": {"stable": {}},
    "config": {
      "code_id": 79,
      "total_fee_bps": 100,
      "maker_fee_bps": 2000,
      "pool_creation_fee": "1000000",
      "is_disabled": false,
      "is_generator_disabled": false,
      "permissioned": false
    }
  }
}' --from owner --gas 200000 --fees 1000uzig
```

#### Step 2: Create Stable Pair

**Important**: The `init_params` must be base64-encoded. Here's how to create it:

```bash
# Create the init_params JSON and encode to base64
echo '{"amp": 100}' | base64
# Output: eyJhbXAiOjEwMH0=
```

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
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig
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
    ],
    "slippage_tolerance": "0.005"
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000usdc
```

## 🎯 Concentrated Pairs

### Overview

Concentrated pairs use advanced mathematical formulas to provide more efficient liquidity provision compared to standard XYK pairs. These are not Uniswap V3 clones and do not use fee tiers or ticks. Instead, they use sophisticated algorithms with configurable parameters for amplification, gamma, fees, and price scaling.

### Key Features

- **Advanced Algorithms**: Uses sophisticated mathematical models for efficient trading
- **Custom Parameters**: Configurable amplification, gamma, fees, price scaling, and other parameters
- **Efficient Trading**: Optimized for specific trading scenarios with dynamic fee adjustment
- **No Fee Tiers or Ticks**: Uses dynamic fee adjustment based on pool balance and price scale
- **Price Oracle**: Built-in price oracle with configurable half-time for price averaging

### Create Concentrated Pair

**Important**: The `init_params` must be base64-encoded. Here's how to create it:

```bash
# Create the init_params JSON and encode to base64
echo '{
  "amp": "40.0",
  "gamma": "0.0001",
  "mid_fee": "0.005",
  "out_fee": "0.01",
  "fee_gamma": "0.001",
  "repeg_profit_threshold": "0.0001",
  "min_price_scale_delta": "0.000001",
  "price_scale": "1.5",
  "ma_half_time": 600,
  "track_asset_balances": false
}' | base64
# Output: eyJhbXAiOiI0MC4wIiwiZ2FtbWEiOiIwLjAwMDEiLCJtaWRfZmVlIjoiMC4wMDUiLCJvdXRfZmVlIjoiMC4wMSIsImZlZV9nYW1tYSI6IjAuMDAxIiwicmVwZWdfcHJvZml0X3RocmVzaG9sZCI6IjAuMDAwMSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAxIiwicHJpY2Vfc2NhbGUiOiIxLjUiLCJtYV9oYWxmX3RpbWUiOjYwMCwidHJhY2tfYXNzZXRfYmFsYW5jZXMiOmZhbHNlfQ==
```

```bash
zigchaind tx wasm execute <factory_address> '{
  "create_pair": {
    "pair_type": {"concentrated": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"token": {"contract_addr": "zig1..."}}
    ],
    "init_params": "eyJhbXAiOiI0MC4wIiwiZ2FtbWEiOiIwLjAwMDEiLCJtaWRfZmVlIjoiMC4wMDUiLCJvdXRfZmVlIjoiMC4wMSIsImZlZV9nYW1tYSI6IjAuMDAxIiwicmVwZWdfcHJvZml0X3RocmVzaG9sZCI6IjAuMDAwMSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAxIiwicHJpY2Vfc2NhbGUiOiIxLjUiLCJtYV9oYWxmX3RpbWUiOjYwMCwidHJhY2tfYXNzZXRfYmFsYW5jZXMiOmZhbHNlfQ=="
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig
```

**Init Params (Base64 encoded):**

```json
{
  "amp": "40.0",
  "gamma": "0.0001",
  "mid_fee": "0.005",
  "out_fee": "0.01",
  "fee_gamma": "0.001",
  "repeg_profit_threshold": "0.0001",
  "min_price_scale_delta": "0.000001",
  "price_scale": "1.5",
  "ma_half_time": 600,
  "track_asset_balances": false
}
```

### Add Liquidity to Concentrated Pair

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "amounts": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"token": {"contract_addr": "zig1..."}}, "amount": "1000000"}
    ],
    "max_spread": "0.1"
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000uzig
```

## 📊 Common Operations

### Query Pool Information

```bash
zigchaind query wasm contract-state smart <pair_address> '{"pool": {}}' --node <rpc_url> --chain-id <chain_id>
```

### Query Pool Configuration (for Stable Pairs)

```bash
zigchaind query wasm contract-state smart <pair_address> '{"config": {}}' --node <rpc_url> --chain-id <chain_id>
```

**Example Output for Stable Pair:**
```json
{
  "block_time_last": 1758398317,
  "factory_addr": "<factory_address>",
  "owner": "<owner_address>",
  "params": "eyJhbXAiOiIxMDAiLCJmZWVfc2hhcmUiOm51bGx9"
}
```

**Decode AMP Parameter:**
```bash
# The params field is base64-encoded
echo "eyJhbXAiOiIxMDAiLCJmZWVfc2hhcmUiOm51bGx9" | base64 -d
# Output: {"amp":"100","fee_share":null}
```

### Query Liquidity Provider Balance

```bash
zigchaind query wasm contract-store <pair_address> '{"balance": {"address": "zig1..."}}' --node <rpc_url> --chain-id <chain_id>
```

### Remove Liquidity

```bash
zigchaind tx wasm execute <pair_address> '{
  "withdraw_liquidity": {
    "amount": "1000000"
  }
}' --from user --gas auto --fees 1000uzig
```

## 🔧 Configuration

### Fee Structure

- **XYK Pairs**: Configurable swap fee
- **Stable Pairs**: Lower fees for stable assets
- **Concentrated Pairs**: Configurable fee rate set at creation (see init params)

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

## 🔧 Base64 Encoding Helper

### Creating Init Params

For stable and concentrated pairs, you need to base64-encode the init parameters:

```bash
# Stable pair init params
echo '{"amp": 100}' | base64
# Output: eyJhbXAiOjEwMH0=

# Concentrated pair init params
echo '{
  "amp": "40.0",
  "gamma": "0.0001",
  "mid_fee": "0.005",
  "out_fee": "0.01",
  "fee_gamma": "0.001",
  "repeg_profit_threshold": "0.0001",
  "min_price_scale_delta": "0.000001",
  "price_scale": "1.5",
  "ma_half_time": 600,
  "track_asset_balances": false
}' | base64
# Output: eyJhbXAiOiI0MC4wIiwiZ2FtbWEiOiIwLjAwMDEiLCJtaWRfZmVlIjoiMC4wMDUiLCJvdXRfZmVlIjoiMC4wMSIsImZlZV9nYW1tYSI6IjAuMDAxIiwicmVwZWdfcHJvZml0X3RocmVzaG9sZCI6IjAuMDAwMSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAxIiwicHJpY2Vfc2NhbGUiOiIxLjUiLCJtYV9oYWxmX3RpbWUiOjYwMCwidHJhY2tfYXNzZXRfYmFsYW5jZXMiOmZhbHNlfQ==
```

### Online Tools

You can also use online base64 encoders:

1. Copy your JSON init params
2. Go to a base64 encoder (e.g., base64encode.org)
3. Paste the JSON and encode
4. Use the encoded string in your transaction

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
- **[Claim Rewards](./incentives.md#claim-rewards)** - Claim earned rewards
- **[Pool Management](./incentives.md#setup-pools)** - Manage staking pools

### Transaction Index

- **[Complete Transaction Index](../transactions.md)** - All transaction examples in one place

## 🚨 Important Notes

1. **Pool Creation Fee**: 1 ZIG (1,000,000 uzig) required when creating pairs
2. **LP Token Creation Fee**: 100 ZIG (100,000,000 uzig) required by ZIGChain to create a token
3. **Liquidity Provision**: Must provide both assets in correct ratios
4. **Slippage Protection**: Always use appropriate slippage limits
5. **Base64 Encoding**: Init params for stable and concentrated pairs must be base64-encoded
6. **Message Names**: Use `provide_liquidity` (not `add_liquidity`) for adding liquidity

## 📈 Performance Considerations

- **XYK**: Best for general trading, simple to understand
- **Stable**: Best for stablecoin pairs, lower slippage
- **Concentrated**: Best for advanced trading scenarios with custom parameters
