---
layout: default
title: Events Documentation
---

# Events Documentation

This section contains documentation for all events emitted by Oroswap contracts.

## 📋 Overview

Events are emitted by contracts to provide transparency and allow external systems to track contract state changes. This documentation covers all events from the main Oroswap contracts.

## 🔗 Event Documentation

### [Factory Events](./factory-events.md)
Events emitted by the factory contract including:
- Pair creation events
- Configuration updates
- Pause/unpause events
- Authority management events

### [Pair Events](./pair-events.md)
Events emitted by pair contracts including:
- Swap events with reserves information
- Liquidity provision and withdrawal events
- Concentrated liquidity events
- Stable pool amplification updates

### [Router Events](./router-events.md)
Events during router operations:
- Multi-hop swap tracking
- CW20 token receive events
- Pair event sequences for complex swaps

## 📊 Event Monitoring

### Key Event Types
- `wasm-pair_created` - New pair creation
- `wasm-swap` - Token swap execution
- `wasm-provide_liquidity` - Liquidity addition
- `wasm-withdraw_liquidity` - Liquidity removal

### Query Examples
```bash
# Get recent pair creations
zigchaind query txs --events 'wasm-pair_created.contract_address=<factory_address>' --limit 10

# Get swap events
zigchaind query txs --events 'wasm-swap.action=swap' --limit 10

# Get liquidity events
zigchaind query txs --events 'wasm-provide_liquidity.action=provide_liquidity' --limit 10
```

## 🔍 Event Parsing

### JavaScript Example
```javascript
// Parse swap event
function parseSwapEvent(event) {
  const attributes = event.attributes;
  const offerAsset = JSON.parse(attributes.find(attr => attr.key === 'offer_asset').value);
  const askAsset = JSON.parse(attributes.find(attr => attr.key === 'ask_asset').value);
  
  return {
    offerAsset,
    askAsset,
    commission: parseInt(attributes.find(attr => attr.key === 'commission_amount').value),
    spread: parseInt(attributes.find(attr => attr.key === 'spread_amount').value)
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
        'spread_amount': int(attributes['spread_amount'])
    }
```

## 🚨 Important Notes

1. **Event Order**: Events are emitted in the order of execution
2. **JSON Parsing**: Asset information is stored as JSON strings
3. **Amount Precision**: All amounts are in the smallest unit (e.g., uzig)
4. **Address Format**: All addresses are in bech32 format
5. **Event Types**: All events have the "wasm-" prefix
6. **Reserves Information**: Swap events include current pool reserves

## 🔗 Related Documentation

- **[Main Documentation](../index.md)** - Return to main documentation
- **[Transaction Index](../transactions/)** - Complete transaction reference
- **[Contract Documentation](../contracts/)** - Contract-specific documentation 