# Factory Events

The Factory contract emits various events to track pair creation, configuration changes, and administrative actions.

## 📋 Event Overview

All factory events include standard attributes:
- `contract_address`: The factory contract address
- `action`: The action performed
- `pair_address`: The created pair address (for pair-related events)

## 🔧 Core Events

### PairCreated
Emitted when a new trading pair is created.

**Event Type**: `wasm-pair_created`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "create_pair",
  "pair_address": "zig1...",
  "asset_infos": "[{\"native_token\":{\"denom\":\"uzig\"}},{\"native_token\":{\"denom\":\"usdc\"}}]",
  "pair_type": "{\"xyk\":{}}",
  "total_fee_bps": "10",
  "maker_fee_bps": "2000",
  "pool_creation_fee": "1000000",
  "creator": "zig1..."
}
```

**Pool Creation Fee**: The event includes the pool creation fee (1,000,000 uzig) and LP token creation fee (100,000,000 uzig) that were paid to create the pair.

**Example Query**:
```bash
zigchaind query txs --events 'wasm-pair_created.contract_address=<factory_address>' --limit 10
```

### ConfigUpdated
Emitted when factory configuration is updated.

**Event Type**: `wasm-config_updated`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "update_config",
  "fee_address": "zig1...",
  "generator_address": "zig1...",
  "coin_registry_address": "zig1..."
}
```

### PairConfigUpdated
Emitted when pair configuration is updated.

**Event Type**: `wasm-pair_config_updated`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "update_pair_config",
  "pair_type": "{\"xyk\":{}}",
  "total_fee_bps": "30",
  "maker_fee_bps": "2000",
  "pool_creation_fee": "1000000"
}
```

## 🔒 Pause Events

### PairPaused
Emitted when a pair is paused.

**Event Type**: `wasm-pair_paused`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "pause_pair",
  "asset_infos": "[{\"native_token\":{\"denom\":\"uzig\"}},{\"native_token\":{\"denom\":\"usdc\"}}]",
  "pair_type": "{\"xyk\":{}}",
  "paused_by": "zig1..."
}
```

### PairUnpaused
Emitted when a pair is unpaused.

**Event Type**: `wasm-pair_unpaused`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "unpause_pair",
  "asset_infos": "[{\"native_token\":{\"denom\":\"uzig\"}},{\"native_token\":{\"denom\":\"usdc\"}}]",
  "pair_type": "{\"xyk\":{}}",
  "unpaused_by": "zig1..."
}
```

## 👥 Authority Events

### PauseAuthorityAdded
Emitted when a pause authority is added.

**Event Type**: `wasm-pause_authority_added`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "add_pause_authorities",
  "authorities": "[\"zig1...\",\"zig1...\"]",
  "added_by": "zig1..."
}
```

### PauseAuthorityRemoved
Emitted when a pause authority is removed.

**Event Type**: `wasm-pause_authority_removed`

**Attributes**:
```json
{
  "contract_address": "<factory_address>",
  "action": "remove_pause_authorities",
  "authorities": "[\"zig1...\"]",
  "removed_by": "zig1..."
}
```

## 🔍 Querying Events

### Get Recent Pair Creations
```bash
zigchaind query txs \
  --events 'wasm-pair_created.contract_address=<factory_address>' \
  --limit 20 \
  --output json
```

### Get Pause Events
```bash
zigchaind query txs \
  --events 'wasm-pair_paused.contract_address=<factory_address>' \
  --limit 10
```

### Get Config Updates
```bash
zigchaind query txs \
  --events 'wasm-config_updated.contract_address=<factory_address>' \
  --limit 5
```

## 📊 Event Indexing

### For Frontend Integration
```javascript
// Example: Listen for new pair creations
const query = `wasm-pair_created.contract_address=${FACTORY_ADDRESS}`;
const events = await client.queryTxs(query, { limit: 10 });

events.txs.forEach(tx => {
  const pairCreated = tx.events.find(e => e.type === 'wasm-pair_created');
  if (pairCreated) {
    const pairAddress = pairCreated.attributes.find(a => a.key === 'pair_address');
    console.log('New pair created:', pairAddress.value);
  }
});
```

### For Analytics
```bash
# Count total pairs created
zigchaind query txs \
  --events 'wasm-pair_created.contract_address=<factory_address>' \
  --limit 1000 | grep -c "pair_created"
```

## 🔗 Related Events

- [Pair Events](./pair-events.md) - Events from individual pair contracts
- [Router Events](./router-events.md) - Events from router contract
- [Incentives Events](./incentives-events.md) - Events from incentives contract 