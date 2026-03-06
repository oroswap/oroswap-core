# Oroswap Airdrop Contract

This contract manages token airdrops with a configurable vesting schedule and an APY-based incentive layer. It allows admins to configure per-user allocations and offers users choice between immediate partial liquidity or full vesting with bonuses.

## Features

- **Dual-Path Claiming**:
    - **Initial Portion**: Claim a configurable percentage (e.g., 40%) immediately. The remaining amount is forfeited.
    - **Full Vesting**: Elect to vest 100% of the allocation to accrue APY-based bonuses.
- **Linear Vesting**: Vested tokens unlock linearly over a configurable duration.
- **APY Bonus**: Tokens held in the vesting pool accrue an APY-based bonus, calculated on the remaining principal.
- **Multi-Token Support**: Supports native and TokenFactory denoms.

## ExecuteMsg

### `AddAirdrop`
Admin only. Creates a new airdrop record with specific eligibility and vesting rules.

```json
{
  "add_airdrop": {
    "token_address": "native.oro",
    "start_time": 1710000000,
    "end_time": 1710003600,
    "vesting_duration": 15552000,
    "initial_claim_percent": 40,
    "apy_bonus_bps": 1000,
    "eligibility": [
      ["addr1...", "1000000000"],
      ["addr2...", "2000000000"]
    ]
  }
}
```

### `Claim`
Users must call this before `end_time` to participate.

```json
{
  "claim": {
    "airdrop_id": 1,
    "choice": "initial_portion" 
  }
}
```
*Note: choice can be `"initial_portion"` or `"full_vesting"`.*

### `WithdrawVested`
Withdraws currently unlocked tokens plus the accrued APY bonus.

```json
{
  "withdraw_vested": {
    "airdrop_id": 1
  }
}
```

## QueryMsg

### `Airdrop`
Returns the configuration and status of a specific airdrop.

### `ClaimStatus`
Returns the detailed claim record for a user, including eligibile amount and vesting progress.

```json
{
  "claim_status": {
    "id": 1,
    "address": "addr1..."
  }
}
```
