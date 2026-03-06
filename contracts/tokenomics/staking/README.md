# Oroswap xORO Staking (Native)

This contract allows ORO holders to stake their native tokens in exchange for xORO (a TokenFactory-denominated stake). The value of xORO relative to ORO increases over time as protocol rewards are injected into the contract, naturally increasing the exchange rate.

## Features

- **Native Token Staking**: Fully compatible with ZigChain's native `uzig`/`ORO` denoms.
- **Auto-Compounding**: Rewards increase the value of all xORO shares simultaneously.
- **Unbonding Period**: Secure unstaking with a configurable unbonding duration.
- **TokenFactory Integration**: Automatically creates and manages the `xoro` denom via the chain's TokenFactory module.

---

## InstantiateMsg

Initializes the contract and bootstraps the initial liquidity pool.

```json
{
  "deposit_token_denom": "coin.zig1...",
  "token_factory_addr": "zig1qmy2...",
  "bootstrap_amount": "1000",
  "unbonding_duration": 86400
}
```

## ExecuteMsg

### `Stake`
Deposits native ORO tokens into the contract to receive xORO shares.
**Note**: You must send the `ORO` tokens along with the transaction.

```json
{
  "stake": {
    "receiver": "zig1..."
  }
}
```

### `Leave`
Burns xORO shares to begin the unstaking process. The underlying ORO will be available for claiming after the unbonding duration.

```json
{
  "leave": {}
}
```

### `Claim`
Withdraws all matured unbonding entries (tokens that have finished their unbonding period).

```json
{
  "claim": {}
}
```

---

## QueryMsg

### `State`
Returns the current exchange rate and pool health metrics.

```json
{
  "state": {}
}
```

**Response Example:**
```json
{
  "total_shares": "1000000",
  "total_deposit": "1050000",
  "exchange_rate": "1.05"
}
```

### `Config`
Returns general contract settings.

### `StakingInfo`
Returns the current staking and unbonding status for a specific user.

```json
{
  "staking_info": { "address": "zig1..." }
}
```
