# Oroswap Airdrop Contract

Per-user token allocations with an immediate-claim portion, linear vesting on
the remainder, and an optional cliff "APY bonus" paid out only if claimed at
or after `vesting_end_time`. Supports native / IBC denoms and CW20 tokens.

## Features

- **Per-user allocation map.** Admin enrols recipients via `(address, amount)`
  pairs, batched up to `MAX_ELIGIBILITY_BATCH = 200` entries per call (call
  `UpdateEligibility` repeatedly to enrol larger drops).
- **Initial-claim portion.** Configurable percentage (0–100%) unlocks the
  moment `start_time` is reached.
- **Linear vesting on the remainder** between `vesting_start_time` and
  `vesting_end_time`.
- **Cliff APY bonus.** A bonus (capped at `MAX_APY_BONUS_BPS = 10_000`, i.e.
  100%) is added to the payout *only* if the user claims at or after
  `vesting_end_time`. Reward for waiting.
- **Single-settlement claim.** `Claim` is one-shot — the user receives
  initial + currently-vested + (cliff bonus if past end). After that the
  rest of their allocation is forfeit. (Documented UX choice.)
- **Optional `expiry_time`.** After expiry, claims are blocked. Admin can
  reclaim the leftover via `WithdrawFunds` (see below).
- **Native + CW20 support.** Set `is_native: bool` per airdrop. CW20
  deposits use `TransferFrom` (recipient must call `IncreaseAllowance` on
  the CW20 first).
- **Solvency awareness.** O(1) `Solvency` query returns committed
  eligibility, claimed-so-far, current contract balance, and a worst-case
  funding flag.

## Authorization model

| Role | Powers |
|---|---|
| `admin` (in `Config`) | Add/toggle airdrops, update eligibility (before start), update APY bonus (free pre-start, increase-only mid-vesting, locked post-vesting), deposit/withdraw funds, transfer ownership |
| Anyone | Claim their own allocation; read queries |

Admin can be rotated via the standard two-step
`ProposeNewOwner` → `ClaimOwnership` flow.

## ExecuteMsg

### `AddAirdrop`
Admin only. Creates an airdrop and enrols the first batch (≤ 200 entries).

```json
{
  "add_airdrop": {
    "token_address": "coin.<contract>.oro",
    "is_native": true,
    "start_time": 1710000000,
    "vesting_start_time": 1710000000,
    "vesting_end_time": 1711555200,
    "initial_claim_percent": 40,
    "apy_bonus_bps": 1000,
    "eligibility": [
      ["zig1addr1...", "1000000000"],
      ["zig1addr2...", "2000000000"]
    ],
    "expiry_time": 1720000000
  }
}
```

Validation: `start_time ≥ now`, `start_time ≤ vesting_start_time < vesting_end_time`,
`initial_claim_percent ≤ 100`, `apy_bonus_bps ≤ 10_000`,
`expiry_time > vesting_end_time` (if set), `eligibility.len() ≤ 200`.

Duplicate addresses in the same `eligibility` batch are deduped in-memory
with last-wins semantics — the running `Solvency.total_eligibility`
reflects what actually lands in storage, never the inflated sum.

### `UpdateEligibility`
Admin only, **before `start_time`**. Adds or overwrites entries (no remove
op — set an amount to a new value to revise). Same 200-entry cap per call;
chain multiple calls for large airdrops. Overwrites correctly maintain
the running total used by `Solvency`.

```json
{
  "update_eligibility": {
    "airdrop_id": 1,
    "eligibility": [["zig1addr3...", "500000000"]]
  }
}
```

### `UpdateApyBonus`
Admin only. Adjust the cliff bonus rate for a season. Two-phase gate:

| Phase | Direction allowed |
|---|---|
| `now < start_time` | any direction (configuration fix) |
| `start_time ≤ now < vesting_end_time` | **increase only** (`new > old`) |
| `now ≥ vesting_end_time` | rejected — some users may have already claimed at the old rate |

Rationale: once the season opens, users who chose to wait through vesting
made that decision based on the advertised bonus. A decrease would punish
them with no way to un-make the choice. Increases are always safe — they
can only make holders better off.

Any increase is additionally gated by a **funding check**: the resulting
worst-case payout (`outstanding + max_bonus_at_new_bps`) must already be
backed by the contract's current token balance, otherwise the call is
rejected with `ApyUpdateUnderfunded { required, available }`. Top up via
`DepositFunds` before raising the rate.

The new value is still capped at `MAX_APY_BONUS_BPS = 10_000`.

```json
{ "update_apy_bonus": { "airdrop_id": 1, "apy_bonus_bps": 1500 } }
```

Possible errors:
- `Unauthorized` — non-admin caller.
- `ApyBonusTooHigh { got, max }` — new value above the 10_000 bps cap.
- `ApyDecreaseNotAllowed { old, new }` — decrease attempted after start.
- `ApyUpdateAfterVestingEnd {}` — any update attempted post-vesting.
- `ApyUpdateUnderfunded { required, available }` — increase not backed by
  current contract balance.

### `Claim`
User only. **One-shot settlement** — pays out everything the user is
entitled to right now and marks the airdrop claimed:

```
total = (eligible × initial_claim_percent / 100)               // initial
      + (vesting_base × elapsed_in_vesting / vesting_duration)  // linear unlocked
      + (vesting_base × apy_bonus_bps / 10_000  if now ≥ vesting_end_time)
```

where `vesting_base = eligible - initial`. Any portion that wasn't unlocked
yet is forfeit. Users who want the full payout + bonus should wait until
`vesting_end_time` before claiming.

```json
{ "claim": { "airdrop_id": 1 } }
```

### `ToggleAirdrop`
Admin only. Enable/disable a specific airdrop's claim path. Combined with
`expiry_time` this is the pause-then-reclaim flow for `WithdrawFunds`.

```json
{ "toggle_airdrop": { "id": 1, "active": false } }
```

### `DepositFunds`
Admin only. Fund the contract for an airdrop.

- **Native:** attach the coin via `--amount`. Uses `must_pay` to enforce
  exactly the right denom + amount.
- **CW20:** requires the admin to first call `IncreaseAllowance` on the
  CW20 with the airdrop contract as the spender. The contract then issues
  `TransferFrom` to pull the tokens in.

```json
{ "deposit_funds": { "token_address": "...", "amount": "1000000", "is_native": true } }
```

### `WithdrawFunds`
Admin only — **restricted**. Refuses to drain a token while any airdrop
using that token is:

1. still `is_active`, OR
2. has no `expiry_time` configured, OR
3. has not yet reached its `expiry_time`.

The admin must first `ToggleAirdrop { active: false }` AND wait until
`now ≥ expiry_time` before they can recover leftover funds. Tokens not
referenced by any airdrop can be withdrawn freely.

Returns `WithdrawalBlocked { airdrop_id }` or `WithdrawalBlockedNoExpiry { airdrop_id }`
with the conflicting airdrop's id, so the operator knows which one to
deactivate.

### Ownership transfer

Standard two-step:

```json
{ "propose_new_owner": { "owner": "zig1new...", "expires_in": 86400 } }
{ "claim_ownership": {} }
{ "drop_ownership_proposal": {} }
```

## QueryMsg

| Query | Returns |
|---|---|
| `Config {}` | The current admin |
| `Airdrop { id }` | Full config + status for one airdrop |
| `LatestAirdropId {}` | The most recently created airdrop's id (1-indexed) |
| `UserAirdropStatus { id, address }` | Per-user view: total allocation, current claimable, claim state, vesting phase |
| `Solvency { id }` | See below |

### `Solvency`

```json
{ "solvency": { "id": 1 } }
```

Returns:

| Field | Meaning |
|---|---|
| `total_eligibility` | Σ amounts enrolled via `AddAirdrop` / `UpdateEligibility` |
| `total_claimed` | Σ payouts actually emitted across all `Claim` calls so far |
| `outstanding` | `total_eligibility − total_claimed` — minimal still-payable obligation |
| `max_outstanding` | Worst case: every remaining user claims at vesting end and gets the cliff bonus |
| `contract_balance` | Current chain balance held by the contract for `token_address` (native via bank query, CW20 via the CW20 `Balance` query) |
| `is_fully_funded` | `contract_balance ≥ max_outstanding` |

Use this from the front-end / ops dashboards to verify the pool is fully
backed before announcing the airdrop. Both running totals are maintained
in O(1), so the query is cheap even for very large airdrops.

## Constraints

| Constant | Value | Purpose |
|---|---|---|
| `MAX_ELIGIBILITY_BATCH` | 200 | Per-tx cap on `eligibility` vector length. Total airdrop size is unbounded — chain multiple calls. |
| `MAX_APY_BONUS_BPS` | 10_000 (100%) | Sanity cap on cliff bonus. Prevents misconfig where committed obligations dwarf the pool. |

## Migration

Standard cw2 same-name guard. `cw2::set_contract_version` re-stamps the
version on migrate; a name mismatch returns `MigrationError`.
