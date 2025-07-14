# Pair Creation Examples

This guide shows how to create different types of pairs on Oroswap, including XYK, Stable, and Concentrated liquidity pairs.

## 📋 Overview

**Factory Contract**: `zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30` (Testnet)

**Pool Creation Fee**: 1 ZIG (1,000,000 uzig) required for all pair types
**LP Token Creation Fee**: 100 ZIG (100,000,000 uzig) required by ZIGChain to create a token

## 🏭 Creating XYK Pairs

### Basic XYK Pair

XYK pairs use the constant product formula (x * y = k) and are suitable for most trading pairs.

```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "pair_type": {"xyk": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "init_params": null
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### XYK Pair with CW20 Token

```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "pair_type": {"xyk": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"token": {"contract_addr": "zig1..."}}
    ],
    "init_params": null
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 🎯 Creating Stable Pairs

### Basic Stable Pair

Stable pairs are optimized for trading between assets with similar values (like stablecoins).

**Step 1: Create Init Params**
```bash
# Create the init_params JSON and encode to base64
echo '{"amp": 100}' | base64
# Output: eyJhbXAiOjEwMH0=
```

**Step 2: Create the Pair**
```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "pair_type": {"stable": {}},
    "asset_infos": [
      {"native_token": {"denom": "usdc"}},
      {"native_token": {"denom": "uoro"}}
    ],
    "init_params": "eyJhbXAiOjEwMH0="
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Stable Pair with Custom Amplification

```bash
# Create init_params with custom amplification
echo '{"amp": 150}' | base64
# Output: eyJhbXAiOjE1MH0=

zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "pair_type": {"stable": {}},
    "asset_infos": [
      {"native_token": {"denom": "usdc"}},
      {"native_token": {"denom": "udai"}}
    ],
    "init_params": "eyJhbXAiOjE1MH0="
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 🎯 Creating Concentrated Pairs

### Basic Concentrated Pair

Concentrated liquidity pairs allow LPs to concentrate their capital within specific price ranges.

**Step 1: Create Init Params**
```bash
# Create the init_params JSON and encode to base64
echo '{
  "amp": "40.0",
  "gamma": "0.0001",
  "mid_fee": "0.005",
  "out_fee": "0.01",
  "fee_gamma": "0.001",
  "repeg_profit_threshold": "0.0001",
  "min_price_scale_delta": "0.000001",
  "price_scale": "1.5",
  "ma_half_time": 600,
  "track_asset_balances": false
}' | base64
# Output: eyJhbXAiOiI0MC4wIiwiZ2FtbWEiOiIwLjAwMDEiLCJtaWRfZmVlIjoiMC4wMDUiLCJvdXRfZmVlIjoiMC4wMSIsImZlZV9nYW1tYSI6IjAuMDAxIiwicmVwZWdfcHJvZml0X3RocmVzaG9sZCI6IjAuMDAwMSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAxIiwicHJpY2Vfc2NhbGUiOiIxLjUiLCJtYV9oYWxmX3RpbWUiOjYwMCwidHJhY2tfYXNzZXRfYmFsYW5jZXMiOmZhbHNlfQ==
```

**Step 2: Create the Pair**
```bash
zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "pair_type": {"concentrated": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ],
    "init_params": "eyJhbXAiOiI0MC4wIiwiZ2FtbWEiOiIwLjAwMDEiLCJtaWRfZmVlIjoiMC4wMDUiLCJvdXRfZmVlIjoiMC4wMSIsImZlZV9nYW1tYSI6IjAuMDAxIiwicmVwZWdfcHJvZml0X3RocmVzaG9sZCI6IjAuMDAwMSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAxIiwicHJpY2Vfc2NhbGUiOiIxLjUiLCJtYV9oYWxmX3RpbWUiOjYwMCwidHJhY2tfYXNzZXRfYmFsYW5jZXMiOmZhbHNlfQ=="
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Concentrated Pair with Custom Parameters

```bash
# Create init_params with custom parameters
echo '{
  "amp": "50.0",
  "gamma": "0.0002",
  "mid_fee": "0.003",
  "out_fee": "0.008",
  "fee_gamma": "0.0005",
  "repeg_profit_threshold": "0.00005",
  "min_price_scale_delta": "0.0000005",
  "price_scale": "2.0",
  "ma_half_time": 300,
  "track_asset_balances": true
}' | base64
# Output: eyJhbXAiOiI1MC4wIiwiZ2FtbWEiOiIwLjAwMDIiLCJtaWRfZmVlIjoiMC4wMDMiLCJvdXRfZmVlIjoiMC4wMDgiLCJmZWVfZ2FtbWEiOiIwLjAwMDUiLCJyZXBlZ19wcm9maXRfdGhyZXNob2xkIjoiMC4wMDAwNSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAwNSIsInByaWNlX3NjYWxlIjoiMi4wIiwibWFfaGFsZl90aW1lIjozMDAsInRyYWNrX2Fzc2V0X2JhbGFuY2VzIjp0cnVlfQ==

zigchaind tx wasm execute zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "create_pair": {
    "pair_type": {"concentrated": {}},
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"token": {"contract_addr": "zig1..."}}
    ],
    "init_params": "eyJhbXAiOiI1MC4wIiwiZ2FtbWEiOiIwLjAwMDIiLCJtaWRfZmVlIjoiMC4wMDMiLCJvdXRfZmVlIjoiMC4wMDgiLCJmZWVfZ2FtbWEiOiIwLjAwMDUiLCJyZXBlZ19wcm9maXRfdGhyZXNob2xkIjoiMC4wMDAwNSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAwNSIsInByaWNlX3NjYWxlIjoiMi4wIiwibWFfaGFsZl90aW1lIjozMDAsInRyYWNrX2Fzc2V0X2JhbGFuY2VzIjp0cnVlfQ=="
  }
}' --from user --gas auto --fees 1000uzig --amount 101000000uzig \
  --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 🔧 Base64 Encoding Helper

### Creating Init Params

For stable and concentrated pairs, you need to base64-encode the init parameters:

```bash
# Stable pair init params
echo '{"amp": 100}' | base64
# Output: eyJhbXAiOjEwMH0=

# Concentrated pair init params
echo '{
  "amp": "40.0",
  "gamma": "0.0001",
  "mid_fee": "0.005",
  "out_fee": "0.01",
  "fee_gamma": "0.001",
  "repeg_profit_threshold": "0.0001",
  "min_price_scale_delta": "0.000001",
  "price_scale": "1.5",
  "ma_half_time": 600,
  "track_asset_balances": false
}' | base64
# Output: eyJhbXAiOiI0MC4wIiwiZ2FtbWEiOiIwLjAwMDEiLCJtaWRfZmVlIjoiMC4wMDUiLCJvdXRfZmVlIjoiMC4wMSIsImZlZV9nYW1tYSI6IjAuMDAxIiwicmVwZWdfcHJvZml0X3RocmVzaG9sZCI6IjAuMDAwMSIsIm1pbl9wcmljZV9zY2FsZV9kZWx0YSI6IjAuMDAwMDAxIiwicHJpY2Vfc2NhbGUiOiIxLjUiLCJtYV9oYWxmX3RpbWUiOjYwMCwidHJhY2tfYXNzZXRfYmFsYW5jZXMiOmZhbHNlfQ==
```

### Online Tools
You can also use online base64 encoders:
1. Copy your JSON init params
2. Go to a base64 encoder (e.g., base64encode.org)
3. Paste the JSON and encode
4. Use the encoded string in your transaction

## 📊 Querying Pair Information

### Get Factory Configuration

```bash
zigchaind query wasm contract-store zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "config": {}
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### List All Pairs

```bash
zigchaind query wasm contract-store zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "pairs": {
    "start_after": null,
    "limit": 30
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

### Get Specific Pair Information

```bash
zigchaind query wasm contract-store zig17a7mlm84taqmd3enrpcxhrwzclj9pga8efz83vrswnnywr8tv26s7mpq30 '{
  "pair": {
    "asset_infos": [
      {"native_token": {"denom": "uzig"}},
      {"native_token": {"denom": "usdc"}}
    ]
  }
}' --node https://testnet-rpc.zigchain.com --chain-id zig-test-2
```

## 🚨 Important Notes

1. **Pool Creation Fee**: 1 ZIG (1,000,000 uzig) required for creating pairs
2. **LP Token Creation Fee**: 100 ZIG (100,000,000 uzig) required by ZIGChain to create a token
3. **Base64 Encoding**: Init params for stable and concentrated pairs must be base64-encoded
4. **Asset Order**: The order of assets in `asset_infos` matters for some operations
5. **Gas Costs**: Complex pair types (stable, concentrated) require more gas
6. **Address Verification**: Always verify contract addresses before use

## 🔗 Related Examples

- **[Liquidity Examples](./liquidity-examples.md)** - Adding/removing liquidity to created pairs
- **[Swap Examples](./swap-examples.md)** - Trading on created pairs
- **[Transaction Index](../transactions.md)** - Complete transaction reference 