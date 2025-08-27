# Maker Contract

The Maker contract is responsible for collecting fees from pairs and swapping them to ORO tokens for distribution to stakers and governance.

## Features

- **Fee Collection**: Collects fees from trading pairs
- **Bridge Swapping**: Swaps collected fees to ORO via bridge tokens
- **Distribution**: Distributes ORO to stakers and governance
- **Keeper Bridge Management**: Allows keepers to manage non-critical token bridges
- **Critical Token Protection**: Owner-only management of critical tokens

## Contract Messages

### Instantiate

```json
{
  "owner": "zig1...",
  "default_bridge": {"native_token": {"denom": "usdc"}},
  "oro_token": {"native_token": {"denom": "uzig"}},
  "factory_contract": "zig1...",
  "staking_contract": "zig1...",
  "governance_contract": "zig1...",
  "governance_percent": "50",
  "max_spread": "0.1",
  "collect_cooldown": "3600",
  "critical_tokens": [
    {"native_token": {"denom": "uzig"}},
    {"native_token": {"denom": "usdc"}},
    {"native_token": {"denom": "usdt"}}
  ]
}
```

### Execute Messages

#### UpdateConfig
Updates contract configuration. Only owner can execute.

```json
{
  "update_config": {
    "factory_contract": "zig1...",
    "staking_contract": "zig1...",
    "governance_contract": {"set": "zig1..."},
    "governance_percent": "50",
    "default_bridge": {"native_token": {"denom": "usdc"}},
    "max_spread": "0.1",
    "collect_cooldown": "3600",
    "oro_token": {"native_token": {"denom": "uzig"}},
    "critical_tokens": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}},
      {"native_token": {"denom": "usdt"}}
    ]
  }
}
```

#### UpdateBridges
Adds or removes bridge tokens. **Owner can manage all bridges. Keepers can only manage non-critical tokens.**

```json
{
  "update_bridges": {
    "add": [
      [
        {"token": {"contract_addr": "zig1..."}},
        {"token": {"contract_addr": "zig1..."}}
      ]
    ],
    "remove": [
      {"token": {"contract_addr": "zig1..."}}
    ]
  }
}
```

#### AddKeeper
Adds an authorized keeper. Only owner can execute.

```json
{
  "add_keeper": {
    "keeper": "zig1..."
  }
}
```

#### RemoveKeeper
Removes an authorized keeper. Only owner can execute.

```json
{
  "remove_keeper": {
    "keeper": "zig1..."
  }
}
```

#### Collect
Collects and swaps fee tokens to ORO. Only owner or authorized keepers can execute.

```json
{
  "collect": {
    "assets": [
      {
        "info": {"native_token": {"denom": "usdc"}},
        "amount": "1000000"
      }
    ]
  }
}
```

### Query Messages

#### Config
Returns contract configuration.

```json
{
  "config": {}
}
```

Response:
```json
{
  "owner": "zig1...",
  "default_bridge": {"native_token": {"denom": "usdc"}},
  "oro_token": {"native_token": {"denom": "uzig"}},
  "factory_contract": "zig1...",
  "staking_contract": "zig1...",
  "governance_contract": "zig1...",
  "governance_percent": "50",
  "max_spread": "0.1",
  "authorized_keepers": ["zig1...", "zig1..."],
  "critical_tokens": [
    {"native_token": {"denom": "uzig"}},
    {"native_token": {"denom": "usdc"}},
    {"native_token": {"denom": "usdt"}}
  ]
}
```

#### Bridges
Returns current bridge configurations.

```json
{
  "bridges": {}
}
```

## Keeper Bridge Management

### Overview
The Maker contract supports delegated bridge management to authorized keepers while maintaining security through critical token protection.

### Security Model
- **Owner**: Can manage all bridges (critical and non-critical tokens)
- **Keepers**: Can only manage bridges for non-critical tokens
- **Critical Tokens**: Protected tokens that only the owner can manage bridges for

### Critical Tokens
Critical tokens are typically major assets like:
- ORO token (UZIG)
- Major stablecoins (USDC, USDT)
- Major cryptocurrencies (wBTC, wETH, ATOM)

### Keeper Operations
Keepers can:
- ✅ Add bridges for non-critical tokens
- ✅ Remove bridges for non-critical tokens
- ❌ Modify bridges for critical tokens (will fail)

### Setup Process
1. **Set Critical Tokens** (Owner only):
   ```bash
   ./keeper_bridge_management.sh setup
   ```

2. **Add Keepers** (Owner only):
   ```bash
   zigchaind tx wasm execute $MAKER_CONTRACT '{
     "add_keeper": {"keeper": "zig1keeper..."}
   }' --from owner
   ```

3. **Keeper Operations**:
   ```bash
   # Add bridge for non-critical token
   ./keeper_bridge_management.sh keeper-add zig1token... zig1bridge...
   
   # Remove bridge for non-critical token
   ./keeper_bridge_management.sh keeper-remove zig1token...
   ```

### Audit Trail
All bridge operations include detailed logging:
- `executed_by`: Address that executed the operation
- `is_owner`: Whether the executor was the owner
- `bridges_added`: Number of bridges added
- `bridges_removed`: Number of bridges removed

### Benefits
- **Operational Efficiency**: Keepers can manage daily bridge operations
- **Security**: Critical tokens remain owner-protected
- **Scalability**: Supports unlimited non-critical token bridges
- **Transparency**: Complete audit trail for all operations

## Testing

Run the integration tests:
```bash
cargo test
```

Test keeper bridge management:
```bash
# Set up critical tokens and keeper
./keeper_bridge_management.sh setup

# Test keeper adding bridge
./keeper_bridge_management.sh keeper-add zig1token... zig1bridge...

# Test keeper removing bridge
./keeper_bridge_management.sh keeper-remove zig1token...

# Test keeper trying to modify critical token (should fail)
./keeper_bridge_management.sh test-fail
```
