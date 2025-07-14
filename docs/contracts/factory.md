# Factory Contract

The Factory contract is the central hub of the Oroswap DEX, responsible for creating and managing all trading pairs.

> 📋 **Quick Reference**: See the [Transaction Index](../transactions.md#factory-contract) for all factory operations.

## 📋 Overview

**Contract Address**: `zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30` (Testnet)

**Purpose**:

- Create new trading pairs
- Manage pair configurations
- Handle fee collection
- Coordinate with incentives system

## 🔧 Core Functions

### Create Pair

Creates a new trading pair for two assets.

```rust
pub fn create_pair(
    &self,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset_infos: Vec<AssetInfo>,
    pair_type: PairType,
) -> Result<Response, ContractError>
```

**Parameters**:

- `asset_infos`: Array of two assets to pair
- `pair_type`: Type of pair (XYK, Concentrated, Stable)

**Pool Creation Fee**: 1,000,000 uzig (1 ZIG) must be sent with the transaction

**LP Token Creation Fee**: 100,000,000 uzig (100 ZIG) must be sent with the transaction as well. (This is a requirement to create a token on ZIGChain)

**Example**:

```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "pair_type": {"xyk": {}}
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig
```

**Note**: The `--amount 101000000uzig` flag sends the required pool creation fee + token creation fee to the factory contract.

### Update Config

Updates factory configuration (owner only).

```rust
pub fn update_config(
    &self,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    fee_address: Option<String>,
    generator_address: Option<String>,
    coin_registry_address: Option<String>,
) -> Result<Response, ContractError>
```

**Example**:

```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "update_config": {
    "fee_address": "zig1...",
    "generator_address": "zig1..."
  }
}' --from owner --gas auto
```

### Update Pair Config

Updates configuration for a specific pair type.

```rust
pub fn update_pair_config(
    &self,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pair_type: PairType,
    config: PairConfig,
) -> Result<Response, ContractError>
```

**Example**:

```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "update_pair_config": {
    "pair_type": {"xyk": {}},
    "config": {
      "code_id": 123,
      "total_fee_bps": 30,
      "maker_fee_bps": 10,
      "is_disabled": false
    }
  }
}' --from owner --gas auto
```

## 📊 Query Functions

### Get Factory Configuration

```bash
zigchaind query wasm contract-store zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{"config": {}}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### List All Pairs

```bash
zigchaind query wasm contract-store zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{"pairs": {"limit": 10}}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Pair Information

```bash
zigchaind query wasm contract-store zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "pair": {
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "pair_type": {"xyk": {}}
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

**Parameters**:

- `asset_infos`: Array of two assets to query pair for
- `pair_type`: Type of pair to query (XYK, Stable, or Concentrated)

## 🔗 Related Examples

### Pair Creation

- **[Create XYK Pair](./pairs.md#create-xyk-pair)** - Create constant product pairs
- **[Create Stable Pair](./pairs.md#create-stable-pair)** - Create stable pairs with amplification
- **[Create Concentrated Pair](./pairs.md#create-concentrated-pair)** - Create concentrated liquidity pairs

### Pair Operations

- **[Add Liquidity](./pairs.md#provide-liquidity)** - Add liquidity to created pairs
- **[Swap Tokens](./pairs.md#swap)** - Trade tokens on pairs
- **[Remove Liquidity](./pairs.md#withdraw-liquidity)** - Withdraw liquidity from pairs

### Advanced Operations

- **[Multi-hop Swaps](./router.md#execute-swap-operations)** - Route through multiple pairs
- **[Stake LP Tokens](./incentives.md#deposit)** - Earn rewards for providing liquidity

### Transaction Index

- **[Complete Transaction Index](../transactions.md)** - All transaction examples in one place

## 🚨 Important Notes

1. **Pool Creation Fee**: 1 ZIG (1,000,000 uzig) required when creating pairs
2. LP Token Creation Fee : 100 ZIG required by ZIGChain to create a token
3. **Admin Only**: Configuration updates require owner privileges
4. **Pair Types**: Support for XYK, Stable, and Concentrated pairs
5. **Fee Structure**: Configurable fees per pair type
6. **Integration**: Factory coordinates with incentives and coin registry contracts
