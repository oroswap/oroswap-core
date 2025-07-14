# Router Events

This document describes the events emitted during router operations in Oroswap.

## 📋 Overview

**Important**: The router contract itself does not emit events. Instead, it orchestrates swaps by calling individual pair contracts, which then emit their own events.

When using the router for multi-hop swaps, you will see events from each pair contract involved in the swap operation.

## 🔄 Router Operations

### Multi-Hop Swap Flow

When executing a multi-hop swap through the router (e.g., ZIG → USDC → ORO), the following events are emitted:

1. **Pair Swap Events**: Each pair contract emits its own `wasm-swap` event
2. **No Router Events**: The router contract does not emit any events

### Example Multi-Hop Swap Events

For a swap ZIG → USDC → ORO, you would see:

```json
// Stage 1: ZIG → USDC (from ZIG/USDC pair)
{
  "type": "wasm-swap",
  "attributes": [
    {"key": "action", "value": "swap"},
    {"key": "offer_asset", "value": "{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000000\"}"},
    {"key": "ask_asset", "value": "{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"950000\"}"},
    {"key": "commission_amount", "value": "3000"},
    {"key": "spread_amount", "value": "47000"},
    {"key": "sender", "value": "zig1..."},
    {"key": "receiver", "value": "zig1..."},
    {"key": "reserves", "value": "{\"native_token\":{\"denom\":\"uzig\"}}:1000000,{\"native_token\":{\"denom\":\"usdc\"}}:950000"}
  ]
}

// Stage 2: USDC → ORO (from USDC/ORO pair)
{
  "type": "wasm-swap",
  "attributes": [
    {"key": "action", "value": "swap"},
    {"key": "offer_asset", "value": "{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"950000\"}"},
    {"key": "ask_asset", "value": "{\"info\":{\"native_token\":{\"denom\":\"uoro\"}},\"amount\":\"900000\"}"},
    {"key": "commission_amount", "value": "2850"},
    {"key": "spread_amount", "value": "47150"},
    {"key": "sender", "value": "zig1..."},
    {"key": "receiver", "value": "zig1..."},
    {"key": "reserves", "value": "{\"native_token\":{\"denom\":\"usdc\"}}:950000,{\"native_token\":{\"denom\":\"uoro\"}}:900000"}
  ]
}
```

## 🎯 CW20 Token Events

### CW20 Receive Event

When CW20 tokens are sent to the router for swapping, a standard CW20 `wasm-receive` event is emitted:

```json
{
  "type": "wasm-receive",
  "attributes": [
    {
      "key": "action",
      "value": "receive"
    },
    {
      "key": "sender",
      "value": "zig1..."
    },
    {
      "key": "amount",
      "value": "1000000"
    },
    {
      "key": "msg",
      "value": "{\"execute_swap_operations\":{\"operations\":[...],\"minimum_receive\":\"950000\"}}"
    }
  ]
}
```

## 🔍 Event Parsing

### JavaScript Example

```javascript
// Parse multi-hop swap events
function parseMultiHopSwap(txEvents) {
  const swapEvents = txEvents.filter(event => event.type === 'wasm-swap');
  
  return swapEvents.map(event => {
    const attributes = event.attributes;
    const offerAsset = JSON.parse(attributes.find(attr => attr.key === 'offer_asset').value);
    const askAsset = JSON.parse(attributes.find(attr => attr.key === 'ask_asset').value);
    
    return {
      offerAsset,
      askAsset,
      commission: parseInt(attributes.find(attr => attr.key === 'commission_amount').value),
      spread: parseInt(attributes.find(attr => attr.key === 'spread_amount').value),
      reserves: attributes.find(attr => attr.key === 'reserves').value
    };
  });
}
```

### Python Example

```python
import json

def parse_multi_hop_swap(tx_events):
    """Parse multi-hop swap events from a transaction"""
    swap_events = [event for event in tx_events if event['type'] == 'wasm-swap']
    
    stages = []
    for event in swap_events:
        attributes = {attr['key']: attr['value'] for attr in event['attributes']}
        
        stages.append({
            'offer_asset': json.loads(attributes['offer_asset']),
            'ask_asset': json.loads(attributes['ask_asset']),
            'commission_amount': int(attributes['commission_amount']),
            'spread_amount': int(attributes['spread_amount']),
            'reserves': attributes['reserves']
        })
    
    return stages
```

## 🚨 Important Notes

1. **No Router Events**: The router contract does not emit any events itself
2. **Pair Events Only**: All swap events come from individual pair contracts
3. **Multi-Hop Tracking**: Track multi-hop swaps by following the sequence of pair swap events
4. **Event Order**: Events are emitted in the order of execution (first pair, then second pair, etc.)
5. **Reserves Information**: Each pair swap event includes current reserves after the swap

## 🔗 Related Events

- **[Pair Events](./pair-events.md)** - Events from individual pair contracts (these are what you'll see)
- **[Factory Events](./factory-events.md)** - Events from factory contract
- **[Incentives Events](./incentives-events.md)** - Events from incentives contract

## 📊 Monitoring

### Key Metrics to Track

1. **Multi-Hop Volume**: Total volume through multi-hop swaps
2. **Route Efficiency**: Average number of pairs used per multi-hop swap
3. **Pair Usage**: Which pairs are most commonly used in multi-hop routes
4. **Slippage Analysis**: Compare slippage across different route lengths

### Example Queries

```bash
# Get all swap events from a specific transaction (multi-hop swap)
zigchaind query tx <tx_hash> --output json | jq '.tx_result.log'

# Get recent multi-hop swaps (transactions with multiple swap events)
zigchaind query txs --events 'wasm-swap.action=swap' --limit 100

# Get swap events for a specific pair
zigchaind query txs --events 'wasm-swap.sender=<pair_address>' --limit 10
```

## 📈 Multi-Hop Swap Analysis

### Identifying Multi-Hop Swaps

To identify multi-hop swaps in transaction logs:

1. **Look for multiple `wasm-swap` events** in the same transaction
2. **Check the sequence** - each swap should use the output of the previous swap
3. **Verify the router contract** is the sender for intermediate swaps
4. **Track the final recipient** - only the last swap should have a non-router recipient

### Example Multi-Hop Transaction

```json
{
  "tx_result": {
    "log": [
      {
        "events": [
          {
            "type": "wasm-swap",
            "attributes": [
              {"key": "action", "value": "swap"},
              {"key": "sender", "value": "zig1..."}, // Router address
              {"key": "receiver", "value": "zig1..."}, // Router address
              {"key": "offer_asset", "value": "{\"info\":{\"native_token\":{\"denom\":\"uzig\"}},\"amount\":\"1000000\"}"}
            ]
          },
          {
            "type": "wasm-swap", 
            "attributes": [
              {"key": "action", "value": "swap"},
              {"key": "sender", "value": "zig1..."}, // Router address
              {"key": "receiver", "value": "zig1..."}, // Final user address
              {"key": "offer_asset", "value": "{\"info\":{\"native_token\":{\"denom\":\"usdc\"}},\"amount\":\"950000\"}"}
            ]
          }
        ]
      }
    ]
  }
}
``` 