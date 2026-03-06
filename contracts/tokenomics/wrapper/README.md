# Oroswap Token Wrapper

The Wrapper contract allows users to lock native or IBC tokens in exchange for 1:1 minted TokenFactory tokens (e.g., locking ORO to receive xORO). It provides a simple mechanism for wrapping/unwrapping while maintaining a strict 1:1 invariant.

## Features

- **1:1 Token Mapping**: Every minted token is backed by exactly one locked native token.
- **TokenFactory Integration**: Automatically manages denom creation and metadata on ZigChain.
- **Pausable**: Admins can pause new locks/unlocks in case of emergency.
- **Receiver Support**: Allows locking/unlocking tokens into a different destination address.

## ExecuteMsg

### `Lock`
Deposit the `lock_denom` and receive the `mint_denom` 1:1.
**Note**: You must send the native tokens along with this call.

```json
{
  "lock": {
    "receiver": "addr1..."
  }
}
```

### `Unlock`
Burn the `mint_denom` and receive the original native `lock_denom` back.
**Note**: You must send the mint_denom tokens along with this call.

```json
{
  "unlock": {
    "receiver": "addr1..."
  }
}
```

### `Pause` / `Unpause`
Owner only. Disables or enables operations.

## QueryMsg

### `Config`
Returns the contract configuration including denoms and owner.

### `TotalLocked`
Returns the total amount of tokens currently locked in the contract (circulating supply of the wrapped token).

```json
{
  "total_locked": {}
}
```
