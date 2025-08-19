# Oroswap Pool Initializer

The Pool Initializer contract provides a way to create pools and add initial liquidity in a single atomic transaction. This eliminates the need for users to sign two separate transactions when setting up new pools.

## 🎯 **Purpose**

Currently, creating a pool requires two separate transactions:
1. **Create Pair** - Call factory to instantiate the pair contract
2. **Add Liquidity** - Call the pair contract to add initial liquidity

This contract combines both operations into a single atomic transaction, providing a better user experience.

## 🚀 **How It Works**

1. **User calls** `InitializePoolWithLiquidity` with pool parameters and initial liquidity
2. **Contract calls** factory to create the pair
3. **Factory replies** with the new pair address
4. **Contract automatically** calls the pair to add initial liquidity
5. **All operations** succeed or fail together atomically

## 📋 **Usage**

### Instantiate the Contract

```bash
zigchaind tx wasm instantiate $POOL_INITIALIZER_CODE_ID '{
  "factory_addr": "'"$FACTORY_ADDR"'"
}' \
  --label "pool-initializer" \
  --admin $ADMIN \
  --from devnet-key --keyring-backend test \
  --node https://devnet-rpc.zigchain.com \
  --chain-id zig-devnet-1 \
  --gas auto --gas-adjustment 1.3 --gas-prices 0.25uzig \
  -y
```

### Initialize Pool with Liquidity

```bash
zigchaind tx wasm execute $POOL_INITIALIZER_ADDR '{
  "initialize_pool_with_liquidity": {
    "pair_type": {"xyk": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "init_params": null,
    "initial_liquidity": [
      {"info": {"native_token": {"denom": "uzig"}}, "amount": "1000000"},
      {"info": {"native_token": {"denom": "usdc"}}, "amount": "1000000"}
    ],
    "slippage_tolerance": "0.01",
    "receiver": "'"$USER_ADDR"'"
  }
}' \
  --amount "2000000uzig,1000000usdc" \
  --from devnet-key --keyring-backend test \
  --node https://devnet-rpc.zigchain.com \
  --chain-id zig-devnet-1 \
  --gas auto --gas-adjustment 1.3 --gas-prices 0.25uzig \
  -y
```

## 🔧 **Parameters**

### `InitializePoolWithLiquidity`

- **`pair_type`** - The type of pair (XYK, Stable, Concentrated, etc.)
- **`asset_infos`** - Array of asset information for the pool
- **`init_params`** - Optional parameters for custom pool types
- **`initial_liquidity`** - Array of assets to add as initial liquidity
- **`slippage_tolerance`** - Optional slippage tolerance (e.g., "0.01" for 1%)
- **`receiver`** - Optional address to receive LP tokens (defaults to sender)

## ✅ **Benefits**

1. **Single Transaction** - User signs only once
2. **Atomic Operation** - Both operations succeed or fail together
3. **Better UX** - Simplified pool creation process
4. **No Contract Changes** - Works with existing audited contracts
5. **Reusable** - Can be used for multiple pool creations

## 🔒 **Security**

- **No Critical Logic** - This is a coordination contract, not core protocol logic
- **Validates Inputs** - Checks asset infos, slippage tolerance, etc.
- **Uses Existing Contracts** - Leverages audited factory and pair contracts
- **Clear Error Handling** - Provides meaningful error messages

## 🧪 **Testing**

```bash
# Run tests
cargo test --package oroswap-pool-initializer

# Run integration tests
cargo test --package oroswap-pool-initializer --test integration
```

## 📦 **Build**

```bash
# Build optimized WASM
cargo build --package oroswap-pool-initializer --release

# Check WASM
cosmwasm-check target/wasm32-unknown-unknown/release/oroswap_pool_initializer.wasm
```
