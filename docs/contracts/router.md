# Router Contract

The Router contract enables complex trading operations including multi-hop swaps, optimal routing, and batch operations across multiple pairs.

> 📋 **Quick Reference**: See the [Transaction Index](../transactions.md#router-contract) for all router operations.

## 📋 Overview

**Contract Address**: `zig1g00t6pxg3xn7vk0vt29zu9vztm3wsq5t5wegutlg94uddju0yr5sye3r3a` (Testnet)

**Purpose**:
- Execute multi-hop swaps across multiple pairs
- Find optimal routes for token swaps
- Handle complex trading operations
- Provide slippage protection for multi-step trades

## 🔄 Multi-Hop Swaps

### Overview
Multi-hop swaps allow trading between tokens that don't have a direct pair by routing through intermediate tokens.

### Execute Swap Operations
The main function for executing multi-hop swaps:

```bash
zigchaind tx wasm execute <router_address> '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uzig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "usdc"}},
          "ask_asset_info": {"token": {"contract_addr": "zig1..."}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "1000000",
    "to": "zig1...",
    "max_spread": "0.01"
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

**Parameters**:
- `operations`: Array of swap operations to execute
- `minimum_receive`: Minimum amount to receive (slippage protection)
- `to`: Recipient address (optional, defaults to sender)
- `max_spread`: Maximum spread allowed (optional)

### Complex Routing Example
```bash
zigchaind tx wasm execute <router_address> '{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uzig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "usdc"}},
          "ask_asset_info": {"native_token": {"denom": "uoro"}},
          "pair_type": {"stable": {}}
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uoro"}},
          "ask_asset_info": {"token": {"contract_addr": "zig1..."}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "500000",
    "max_spread": "0.02"
  }
}' --from user --gas auto --fees 1000uzig --amount 1000000uzig
```

## 🎯 CW20 Token Swaps

### Send CW20 Tokens to Router
```bash
zigchaind tx wasm execute <token_address> '{
  "send": {
    "contract": "<router_address>",
    "amount": "1000000",
    "msg": "eyJleGVjdXRlX3N3YXBfb3BlcmF0aW9ucyI6eyJvcGVyYXRpb25zIjpbeyJvcm9fc3dhcCI6eyJvZmZlcl9hc3NldF9pbmZvIjp7InRva2VuIjp7ImNvbnRyYWN0X2FkZHIiOiJ6aWcxLi4uIn19LCJhc2tfYXNzZXRfaW5mbyI6eyJuYXRpdmVfdG9rZW4iOnsiZGVub20iOiJ1c2RjIn19LCJwYWlyX3R5cGUiOnsieHlrIjp7fX19XSwibWluaW11bV9yZWNlaXZlIjoiMTAwMDAwMCJ9fQ=="
  }
}' --from user --gas auto --fees 1000uzig
```

**Decoded msg:**
```json
{
  "execute_swap_operations": {
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"token": {"contract_addr": "zig1..."}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ],
    "minimum_receive": "1000000"
  }
}
```

## 🔧 Advanced Operations

### Simulate Swap Operations
Before executing a swap, you can simulate it to see the expected output:

```bash
zigchaind query wasm contract-store <router_address> '{
  "simulate_swap_operations": {
    "offer_amount": "1000000",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uzig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ]
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Reverse Simulation
Calculate how much input is needed for a desired output:

```bash
zigchaind query wasm contract-store <router_address> '{
  "reverse_simulate_swap_operations": {
    "ask_amount": "1000000",
    "operations": [
      {
        "oro_swap": {
          "offer_asset_info": {"native_token": {"denom": "uzig"}},
          "ask_asset_info": {"native_token": {"denom": "usdc"}},
          "pair_type": {"xyk": {}}
        }
      }
    ]
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 📊 Supported Operations

### Operation Types

1. **Oroswap Swap**
   ```json
   {
     "oro_swap": {
       "offer_asset_info": {"native_token": {"denom": "uzig"}},
       "ask_asset_info": {"token": {"contract_addr": "zig1..."}},
       "pair_type": {"xyk": {}}
     }
   }
   ```

2. **Native Swap** (for future expansion)
   ```json
   {
     "native_swap": {
       "offer_denom": "uzig",
       "ask_denom": "usdc"
     }
   }
   ```

### Pair Types
- `{"xyk": {}}` - Constant product pairs
- `{"stable": {}}` - Stable pairs
- `{"concentrated": {}}` - Concentrated liquidity pairs

## 🛡️ Slippage Protection

### Minimum Receive
Always specify `minimum_receive` to protect against slippage:

```json
{
  "execute_swap_operations": {
    "operations": [...],
    "minimum_receive": "950000"  // Minimum 950,000 tokens received
  }
}
```

### Max Spread
For additional protection, you can specify maximum spread:

```json
{
  "execute_swap_operations": {
    "operations": [...],
    "minimum_receive": "950000",
    "max_spread": "0.01"  // Maximum 1% spread
  }
}
```

## 🔍 Query Operations

### Get Router Configuration
```bash
zigchaind query wasm contract-store <router_address> '{"config": {}}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Check Factory Address
```bash
zigchaind query wasm contract-store <router_address> '{"config": {}}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2 | jq '.data.config.oroswap_factory'
```

## 🔗 Related Examples

### Basic Trading
- **[Single Pair Swaps](./pairs.md#swap)** - Direct swaps on individual pairs
- **[Add Liquidity](./pairs.md#provide-liquidity)** - Provide liquidity to pairs
- **[Remove Liquidity](./pairs.md#withdraw-liquidity)** - Withdraw liquidity from pairs

### Pair Management
- **[Create Pairs](./factory.md#create-pair)** - Create new trading pairs
- **[Factory Configuration](./factory.md#update-config)** - Update factory settings

### Rewards & Staking
- **[Stake LP Tokens](./incentives.md#deposit)** - Earn rewards for liquidity
- **[Claim Rewards](./incentives.md#claim-rewards)** - Claim earned ORO tokens

### Transaction Index
- **[Complete Transaction Index](../transactions.md)** - All transaction examples in one place

## 🚨 Important Considerations

### Gas Limits
- Multi-hop swaps require more gas than single swaps
- Each operation in the chain consumes additional gas
- Estimate gas carefully for complex routes

### Slippage Accumulation
- Each hop in a multi-hop swap can introduce slippage
- Total slippage compounds across operations
- Use appropriate `minimum_receive` values

### Route Optimization
- Consider gas costs vs. slippage when choosing routes
- Shorter routes may have higher slippage but lower gas costs
- Longer routes may have lower slippage but higher gas costs

### Token Approvals
- For CW20 tokens, ensure the router has sufficient allowance
- Check token allowances before executing swaps

## 📈 Best Practices

1. **Always Simulate First**: Use `simulate_swap_operations` before executing
2. **Set Realistic Slippage**: Don't set `minimum_receive` too high
3. **Consider Gas Costs**: Balance route length with gas efficiency
4. **Monitor Liquidity**: Ensure sufficient liquidity in all pairs
5. **Test Small Amounts**: Start with small amounts for new routes

## 🔧 Error Handling

Common errors and solutions:

- **Insufficient Liquidity**: Check if pairs have enough liquidity
- **Slippage Exceeded**: Reduce `minimum_receive` or use different route
- **Gas Limit Exceeded**: Increase gas limit or simplify route
- **Invalid Route**: Ensure all pairs exist and are active 