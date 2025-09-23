# Oroswap Native Coin Registry

The Native Coin Registry contract maintains a registry of native tokens with their precision information for the Oroswap protocol.

## Purpose

This contract serves as a centralized registry for native token metadata, including:
- Token denominations
- Decimal precision
- Token metadata

## Features

- **Token Registration**: Register native tokens with their precision
- **Metadata Storage**: Store token metadata and configuration
- **Precision Lookup**: Query token precision for calculations
- **Admin Management**: Owner-controlled token registry updates

## Usage

The registry is used by other Oroswap contracts to:
- Validate token precision during swaps
- Calculate proper amounts for liquidity provision
- Ensure consistent token handling across the protocol

## Query Messages

### `config`
Returns the contract configuration.

```json
{
  "config": {}
}
```

### `coins`
Returns the list of registered native coins with their precision.

```json
{
  "coins": {}
}
```

## Execute Messages

### `update_config`
Updates the contract configuration. Only owner can execute.

```json
{
  "update_config": {
    "owner": "zig1..."
  }
}
```

### `update_coins`
Updates the list of registered coins. Only owner can execute.

```json
{
  "update_coins": {
    "coins": [
      {
        "denom": "uzig",
        "precision": 6
      },
      {
        "denom": "usdc",
        "precision": 6
      }
    ]
  }
}
```