# Swap Examples

This guide shows how to perform token swaps on Oroswap DEX using either the Router contract or direct pair contracts.

## 📋 Overview

> **Note**: For contract addresses, see the [oroswap-deployments repository](https://github.com/oroswap/oroswap-deployments).

**Supported Assets**:
- Native tokens (azig, uoro, usdc, etc.)
- CW20 tokens (custom tokens)

## 🔄 Basic Swap

### Router-Based Swap
Use the router contract for swaps with custom routing logic.

```bash
zigchaind tx wasm execute <router_address> '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "azig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "950000",
    "to": "zig1...",
    "max_spread": "0.01"
  }
}' --from user --gas auto --fees 1000000000000000azig --amount 1000000000000000000azig
```

### Direct Pair Swap
Swap directly on a specific pair contract. You need to know the exact pair address.

```bash
zigchaind tx wasm execute <pair_address> '{
  "swap": {
    "offer_asset": {
      "info": {"native_token": {"denom": "azig"}},
      "amount": "1000000000000000000"
    },
    "belief_price": "1000000000000",
    "max_spread": "0.01"
  }
}' --from user --gas auto --fees 1000000000000000azig --amount 1000000000000000000azig
```

**Router vs Direct Pair**:
- **Router**: Requires you to specify the exact route and handles multi-hop swaps
- **Direct Pair**: Simpler for single-hop swaps, but you need to know the pair address

### Multi-Hop Swap
Swap through multiple pairs using the router with custom routing logic.

```bash
zigchaind tx wasm execute <router_address> '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "azig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "usdc"}},
          "ask_asset_info": {"native_token": {"denom": "uoro"}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "900000",
    "to": "zig1...",
    "max_spread": "0.02"
  }
}' --from user --gas auto --fees 2000000000000000azig --amount 1000000000000000000azig
```

**Parameters**:
- `operations`: Array of swap operations to execute
- `minimum_receive`: Minimum amount to receive (slippage protection)
- `to`: Recipient address (optional, defaults to sender)
- `max_spread`: Maximum spread allowed (optional)

## 💰 Fee Calculation

### Understanding Fees
- **Total Fee**: 10 basis points (0.1%)
- **Slippage**: Additional cost due to price impact
- **Gas Fee**: Network transaction fee

### Example Fee Breakdown
```bash
# Swap 1 ZIG for USDC
# Amount: 1,000,000,000,000,000,000 azig (18 decimals)
# Fee: 10 bps = 0.1%
# Slippage: 1%
# Total cost: ~1.1% + gas
```

## 🎯 Advanced Swaps

### CW20 Token Swap
Swap custom tokens.

```bash
zigchaind tx wasm execute <router_address> '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"token": {"contract_addr": "zig1..."}},
          "ask_asset_info": {"native_token": {"denom": "azig"}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "950000000000000000",
    "max_spread": "0.05"
  }
}' --from user --gas auto
```

### Concentrated Liquidity Swap
Swap through concentrated liquidity pools.

```bash
zigchaind tx wasm execute <router_address> '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "azig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"concentrated": {}}
        }
      }
    ],
    "minimum_receive": "950000",
    "max_spread": "0.01"
  }
}' --from user --gas auto --amount 1000000000000000000azig
```

## 📊 Query Swap Simulation

### Simulate Swap
Get expected output before executing.

```bash
zigchaind query wasm contract-state smart <router_address> '{
  "simulate_swap_operations": {
    "offer_amount": "1000000000000000000",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "azig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ]
  }
}'
```

**Response**:
```json
{
  "amount": "950000",
  "router_stages": [
    {
      "stage": 0,
      "pair_address": "zig1...",
      "offer_asset": {"native_token": {"denom": "azig"}},
      "ask_asset": {"native_token": {"denom": "usdc"}},
      "return_amount": "950000",
      "commission_amount": "10000",
      "spread_amount": "50000"
    }
  ]
}
```

### Get Swap Routes
Find optimal swap routes.

```bash
zigchaind query wasm contract-state smart <router_address> '{
  "reverse_simulate_swap_operations": {
    "ask_amount": "1000000",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "azig"}},
          "ask_asset_info": {"native_token": {"denom": "uoro"}},
          "pair_type": {"xyk": {}}
        }
      }
    ]
  }
}'
```

## 🚨 Important Notes

1. **Amount Format**: Always use the full denomination (e.g., `1000000000000000000azig` not just `1000000000000000000`) in the `--amount` flag
2. **Amount in JSON**: Use the raw number (e.g., `"1000000"`) in the JSON message body
3. **Slippage Protection**: Use `minimum_receive` or `max_spread` to prevent front-running
4. **Gas Costs**: Multi-hop swaps require more gas
5. **Token Approval**: CW20 tokens require approval before swapping
6. **Price Impact**: Large swaps may have significant price impact

## 🔗 Related Examples

- **[Liquidity Examples](./liquidity-examples.md)** - Adding/removing liquidity
- **[Pair Creation](./pair-create.md)** - Creating XYK, Stable, and Concentrated pairs
- **[Transaction Index](../transactions.md)** - Complete transaction reference

