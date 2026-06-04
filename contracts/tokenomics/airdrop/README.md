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
- **Mandatory `expiry_time`.** Required on `AddAirdrop` and validated to be
  strictly greater than `vesting_end_time`. After expiry, claims are blocked
  and the admin can reclaim leftover tokens via `WithdrawFunds`. Because
  expiry is mandatory, tokens are never permanently stranded.
- **Native + CW20 support.** Set `is_native: bool` per airdrop. CW20
  deposits use `TransferFrom` (recipient must call `IncreaseAllowance` on
  the CW20 first).
- **Solvency awareness.** O(1) `Solvency` query returns committed
  eligibility, claimed-so-far, current contract balance, and a worst-case
  funding flag.

## Authorization model

| Role | Powers |
|---|---|
| `admin` (in `Config`) | Add/toggle airdrops, update eligibility (before start), update APY bonus (free pre-start, increase-only mid-vesting, locked post-vesting), update timing (pre-start, forward-only), extend expiry (any phase before expiry), deposit/withdraw funds, transfer ownership |
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
`expiry_time > vesting_end_time`, `eligibility.len() ≤ 200`.

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

### `UpdateTimes`
Admin only, **before `start_time`** (pre-start only). Pushes one or more of
the three timing fields forward. All supplied fields must strictly exceed
their current stored values (forward-only). At least one field must be
supplied, and all timing invariants are re-checked after the update.

```json
{
  "update_times": {
    "airdrop_id": 1,
    "start_time": 1710100000,
    "vesting_start_time": 1710100000,
    "vesting_end_time": 1712000000
  }
}
```

All three fields (`start_time`, `vesting_start_time`, `vesting_end_time`) are
optional — supply only the ones you want to change. The omitted fields keep
their current values and still participate in the invariant re-check.

Invariants re-checked after update:
- `start_time ≥ now` (the season must not have started yet)
- `start_time ≤ vesting_start_time < vesting_end_time`
- `expiry_time > vesting_end_time`

Tip: if you need to push `vesting_end_time` past the current `expiry_time`,
call `UpdateExpiry` first to extend the expiry, then call `UpdateTimes`.

Possible errors:
- `Unauthorized` — non-admin caller.
- `AirdropAlreadyStarted {}` — `now ≥ start_time`; timing is locked once the
  season opens.
- `NoTimeFieldsProvided {}` — all three optional fields were omitted.
- `NonForwardUpdate { field, old, new }` — a supplied value does not strictly
  exceed the current stored value.
- `StartTimeInPast {}` — updated `start_time` is already in the past.
- `InvalidTimeRange {}` — invariants `start ≤ vesting_start < vesting_end` or
  `expiry > vesting_end` violated after the proposed changes.

### `UpdateExpiry`
Admin only. Extends the expiry deadline for an airdrop. The new value must
strictly exceed the current `expiry_time` (forward-only). Can be called at
**any phase** — pre-start, mid-vesting, or post-vesting-end — as long as
`now < current expiry_time` (i.e. the expiry window has not already closed).

```json
{ "update_expiry": { "airdrop_id": 1, "expiry_time": 1725000000 } }
```

Use this before calling `UpdateTimes` if you need to push `vesting_end_time`
beyond the current `expiry_time`, since the invariant `expiry > vesting_end`
must always hold.

Possible errors:
- `Unauthorized` — non-admin caller.
- `AirdropExpired {}` — `now ≥ current expiry_time`; once expired the window
  cannot be reopened.
- `NonForwardUpdate { field, old, new }` — the new expiry is not strictly
  greater than the current one.
- `InvalidTimeRange {}` — the new expiry would violate `expiry > vesting_end`
  (should not occur in normal usage since extending expiry only makes the gap
  larger).

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
2. has not yet reached its `expiry_time`.

The admin must first `ToggleAirdrop { active: false }` AND wait until
`now ≥ expiry_time` before they can recover leftover funds. Tokens not
referenced by any airdrop can be withdrawn freely. Because `expiry_time`
is **required** on `AddAirdrop` and validated to be strictly greater than
`vesting_end_time`, every airdrop has a deterministic reclaim window —
leftover tokens are never permanently stranded.

Returns `WithdrawalBlocked { airdrop_id }` with the conflicting airdrop's
id, so the operator knows which one to deactivate.

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

Forward-only via `cw2::ensure_from_older_version`: a contract-name mismatch
or a downgrade (`stored_version > CONTRACT_VERSION`) is rejected with a
generic `StdError` wrapped into `ContractError::Std`; an equal version is
a silent no-op; an older stored version is bumped to `CONTRACT_VERSION`.
