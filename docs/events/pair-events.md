# Pair Events

This document describes the events emitted by Oroswap pair contracts during various operations.

## 📋 Overview

Pair contracts emit events for all major operations including swaps, liquidity provision, and withdrawals. These events help track the state changes and provide transparency.

**Asset Balance Tracking**: Some pairs track asset balances over blocks using SnapshotMap, which allows querying historical balances at specific block heights. This is controlled by the `track_asset_balances` parameter during pair creation.

**Reserves Attribute**: Swap events include a `reserves` attribute that shows the current pool reserves after the swap operation, providing real-time liquidity information. This attribute is included in all pair types (XYK, Stable, and Concentrated).

## 🔄 Swap Events

### Swap Event

Emitted when a swap is executed.

```json
{
  "type": "wasm-swap",
  "attributes": [
    {
      "key": "action",
      "value": "swap"
    },
    {
      "key": "offer_asset",
      "value": "{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000000\"}"
    },
    {
      "key": "ask_asset",
      "value": "{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"950000\"}"
    },
    {
      "key": "commission_amount",
      "value": "3000"
    },
    {
      "key": "spread_amount",
      "value": "47000"
    },
    {
      "key": "sender",
      "value": "zig1..."
    },
    {
      "key": "receiver",
      "value": "zig1..."
    },
    {
      "key": "reserves",
      "value": "{\"native_token\":{\"denom\":\"uzig\"}}:1000000,{\"native_token\":{\"denom\":\"usdc\"}}:950000"
    }
  ]
}
```

**Attributes**:
- `action`: Always "swap"
- `offer_asset`: Asset being offered (JSON string)
- `ask_asset`: Asset being received (JSON string)
- `commission_amount`: Fee amount collected
- `spread_amount`: Spread amount (price impact)
- `sender`: Address initiating the swap
- `receiver`: Address receiving the tokens
- `reserves`: Current pool reserves after the swap (format: "asset1:amount1,asset2:amount2")

## 💧 Liquidity Events

### Provide Liquidity Event

Emitted when liquidity is added to a pool.

```json
{
  "type": "wasm-provide_liquidity",
  "attributes": [
    {
      "key": "action",
      "value": "provide_liquidity"
    },
    {
      "key": "assets",
      "value": "[{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000000\"},{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"1000000\"}]"
    },
    {
      "key": "share",
      "value": "1000000"
    },
    {
      "key": "sender",
      "value": "zig1..."
    },
    {
      "key": "receiver",
      "value": "zig1..."
    }
  ]
}
```

**Attributes**:
- `action`: Always "provide_liquidity"
- `assets`: Array of assets provided (JSON string)
- `share`: LP tokens minted
- `sender`: Address providing liquidity
- `receiver`: Address receiving LP tokens

### Withdraw Liquidity Event

Emitted when liquidity is removed from a pool.

```json
{
  "type": "wasm-withdraw_liquidity",
  "attributes": [
    {
      "key": "action",
      "value": "withdraw_liquidity"
    },
    {
      "key": "withdrawn_share",
      "value": "1000000"
    },
    {
      "key": "refund_assets",
      "value": "[{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000000\"},{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"1000000\"}]"
    },
    {
      "key": "sender",
      "value": "zig1..."
    }
  ]
}
```

**Attributes**:
- `action`: Always "withdraw_liquidity"
- `withdrawn_share`: LP tokens burned
- `refund_assets`: Assets returned to user (JSON string)
- `sender`: Address withdrawing liquidity

## 🎯 Concentrated Liquidity Events

**Note**: Concentrated pairs emit the same swap events as other pair types, including the `reserves` attribute that shows current pool reserves after swaps.

### Add Concentrated Liquidity Event

Emitted when concentrated liquidity is added.

```json
{
  "type": "wasm-add_liquidity",
  "attributes": [
    {
      "key": "action",
      "value": "add_liquidity"
    },
    {
      "key": "lower_tick",
      "value": "1000"
    },
    {
      "key": "upper_tick",
      "value": "2000"
    },
    {
      "key": "amounts",
      "value": "[{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000000\"},{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"1000000\"}]"
    },
    {
      "key": "liquidity",
      "value": "1000000"
    },
    {
      "key": "sender",
      "value": "zig1..."
    }
  ]
}
```

### Collect Fees Event

Emitted when fees are collected from concentrated liquidity.

```json
{
  "type": "wasm-collect_fees",
  "attributes": [
    {
      "key": "action",
      "value": "collect_fees"
    },
    {
      "key": "lower_tick",
      "value": "1000"
    },
    {
      "key": "upper_tick",
      "value": "2000"
    },
    {
      "key": "collected_fees",
      "value": "[{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000\"},{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"1000\"}]"
    },
    {
      "key": "sender",
      "value": "zig1..."
    }
  ]
}
```

## 🔧 Stable Pool Events

### Amplification Update Event

Emitted when stable pool amplification is updated.

```json
{
  "type": "wasm-update_amp",
  "attributes": [
    {
      "key": "action",
      "value": "update_amp"
    },
    {
      "key": "old_amp",
      "value": "100"
    },
    {
      "key": "new_amp",
      "value": "150"
    },
    {
      "key": "next_amp_time",
      "value": "1234567890"
    },
    {
      "key": "sender",
      "value": "zig1..."
    }
  ]
}
```

## 📊 Query Events

### Query Pool Event

Emitted when pool information is queried (for tracking purposes).

```json
{
  "type": "wasm-query_pool",
  "attributes": [
    {
      "key": "action",
      "value": "query_pool"
    },
    {
      "key": "pool_id",
      "value": "zig1..."
    }
  ]
}
```

## 🔍 Event Parsing

### JavaScript Example

```javascript
// Parse swap event
function parseSwapEvent(event) {
  const attributes = event.attributes;
  const offerAsset = JSON.parse(attributes.find(attr => attr.key === 'offer_asset').value);
  const askAsset = JSON.parse(attributes.find(attr => attr.key === 'ask_asset').value);
  const commission = attributes.find(attr => attr.key === 'commission_amount').value;
  const spread = attributes.find(attr => attr.key === 'spread_amount').value;
  
  return {
    offerAsset,
    askAsset,
    commission: parseInt(commission),
    spread: parseInt(spread)
  };
}

// Parse liquidity event
function parseLiquidityEvent(event) {
  const attributes = event.attributes;
  const assets = JSON.parse(attributes.find(attr => attr.key === 'assets').value);
  const share = attributes.find(attr => attr.key === 'share').value;
  
  return {
    assets,
    share: parseInt(share)
  };
}
```

### Python Example

```python
import json

def parse_swap_event(event):
    """Parse swap event attributes"""
    attributes = {attr['key']: attr['value'] for attr in event['attributes']}
    
    return {
        'offer_asset': json.loads(attributes['offer_asset']),
        'ask_asset': json.loads(attributes['ask_asset']),
        'commission_amount': int(attributes['commission_amount']),
        'spread_amount': int(attributes['spread_amount']),
        'sender': attributes['sender'],
        'receiver': attributes['receiver']
    }

def parse_liquidity_event(event):
    """Parse liquidity event attributes"""
    attributes = {attr['key']: attr['value'] for attr in event['attributes']}
    
    return {
        'assets': json.loads(attributes['assets']),
        'share': int(attributes['share']),
        'sender': attributes['sender']
    }
```

## 🚨 Important Notes

1. **Event Order**: Events are emitted in the order of execution
2. **JSON Parsing**: Asset information is stored as JSON strings
3. **Amount Precision**: All amounts are in the smallest unit (e.g., uzig)
4. **Address Format**: All addresses are in bech32 format
5. **Event Types**: All pair events have the "wasm-" prefix
6. **Asset Balance Tracking**: Pairs with `track_asset_balances` enabled can be queried for historical balances using the `asset_balance_at` query
7. **Reserves Format**: The reserves attribute uses the format "asset1:amount1,asset2:amount2" where assets are JSON strings

## 🔗 Related Events

- **[Factory Events](./factory-events.md)** - Events from factory contract
- **[Router Events](./router-events.md)** - Events from router contract
- **[Incentives Events](./incentives-events.md)** - Events from incentives contract

## 📈 Monitoring

### Key Metrics to Track

1. **Swap Volume**: Total volume of swaps
2. **Liquidity Changes**: Net liquidity additions/removals
3. **Fee Collection**: Total fees collected
4. **User Activity**: Number of unique users
5. **Price Impact**: Average spread amounts

### Example Queries

```bash
# Get recent swap events
zigchaind query txs --events 'wasm-swap.action=swap' --limit 10

# Get liquidity events for a specific pair
zigchaind query txs --events 'wasm-provide_liquidity.sender=zig1...' --limit 10

# Get events for a specific block
zigchaind query block <block_height>
``` 