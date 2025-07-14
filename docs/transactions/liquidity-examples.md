# Liquidity Examples

This guide shows how to add and remove liquidity from Oroswap pairs.

## 📋 Overview

**Supported Pair Types**:
- **XYK Pairs**: Constant product AMM
- **Stable Pairs**: Optimized for stablecoin trading
- **Concentrated Pairs**: Concentrated liquidity pools

## 💧 Adding Liquidity

### XYK Pairs

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "assets": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"native_token": {"denom": "usdc"}}, "amount": "1000000"}
    ],
    "slippage_tolerance": "0.01",
    "auto_stake": false
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000uzig
```

**Parameters**:
- `assets`: Array of assets to provide (must be exactly 2)
- `slippage_tolerance`: Maximum allowed slippage (optional)
- `auto_stake`: Automatically stake LP tokens (optional)

### Stable Pairs

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

### Concentrated Pairs

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "amounts": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"native_token": {"denom": "usdc"}}, "amount": "1000000"}
    ],
    "max_spread": "0.01"
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000uzig
```

**Parameters**:
- `amounts`: Assets to provide
- `max_spread`: Maximum spread allowed

## 🏃‍♂️ Removing Liquidity

### XYK and Stable Pairs

```bash
zigchaind tx wasm execute <pair_address> '{
  "withdraw_liquidity": {
    "amount": "1000000"
  }
}' --from user --gas auto --fees 1000uzig
```

### Concentrated Pairs

```bash
zigchaind tx wasm execute <pair_address> '{
  "withdraw_liquidity": {
    "amount": "1000000"
  }
}' --from user --gas auto --fees 1000uzig
```

## 📊 Querying Liquidity

### Get LP Token Balance

```bash
zigchaind query wasm contract-store <pair_address> '{
  "balance": {
    "address": "zig1..."
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Pool Information

```bash
zigchaind query wasm contract-store <pair_address> '{
  "pool": {}
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Liquidity Provider Info

```bash
zigchaind query wasm contract-store <pair_address> '{
  "liquidity_provider": {
    "address": "zig1..."
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 🎯 Advanced Features

### Auto-Staking

Automatically stake LP tokens when providing liquidity:

```bash
zigchaind tx wasm execute <pair_address> '{
  "provide_liquidity": {
    "assets": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"native_token": {"denom": "usdc"}}, "amount": "1000000"}
    ],
    "auto_stake": true
  }
}' --from user --gas auto --fees 1000uzig --amount 2000000uzig
```

## 💰 Fee Structure

### XYK Pairs
- **Swap Fee**: 0.1% (configurable)
- **LP Fee**: 0.1% (goes to liquidity providers)

### Stable Pairs
- **Swap Fee**: 0.04% (lower for stable assets)
- **LP Fee**: 0.04% (goes to liquidity providers)

### Concentrated Pairs
- **Swap Fee**: Configurable at creation (set in init params)
- **LP Fee**: Based on configured fee rate

## 🚨 Important Notes

1. **Equal Value**: Assets should be provided in equal USD value
2. **Slippage Protection**: Always use appropriate slippage limits
3. **Gas Costs**: Complex operations require more gas
4. **Minimum Liquidity**: Some pairs have minimum liquidity requirements
5. **Price Impact**: Large liquidity operations may affect prices

## 🔗 Related Examples

- **[Swap Examples](./swap-examples.md)** - How to perform token swaps
- **[Pair Creation](./pair-create.md)** - Creating XYK, Stable, and Concentrated pairs
- **[Transaction Index](../transactions.md)** - Complete transaction reference 