# Oroswap Staking

## Overview

Single-asset staking contract for ORO. Users deposit ORO and the contract
records a per-address balance 1:1 (no share token, no exchange rate). Multi-
denom rewards are distributed pro-rata using a per-share `global_index` /
`user_index` accumulator (same pattern as the incentives contract). Exits
go through a per-user unbonding queue and are released after a configurable
delay.

## Architecture

Storage (`src/state.rs`):

- `STAKES: Map<&Addr, Uint128>` — per-user ORO stake balance. Only changes
  via `Enter` / `Leave`.
- `TOTAL_STAKES: Item<Uint128>` — sum of all `STAKES` entries; denominator
  for reward distribution.
- `REWARDS: Item<Vec<RewardInfo>>` — active reward streams
  (`denom`, `rps`, `index`, `last_update_ts`, `end_ts`).
- `FINISHED_REWARDS: Map<&str, Decimal256>` — frozen final `index` for
  fully-elapsed streams pruned out of `REWARDS`. Lets the
  `MAX_REWARD_DENOMS` cap apply to *concurrent active* streams only, while
  still letting late syncers collect `(finished_index - snapshot) * stake`.
- `USER_REWARDS: Map<&Addr, UserRewardInfo>` — per-user
  `last_rewards_index` snapshot + `pending_rewards` bucket.
- `UNBONDING: Map<&Addr, Vec<UnbondingEntry>>` — per-user queue of
  `{ oro_amount, reward_coins, unlock_time, receiver }`.
- `TOTAL_LOCKED: Item<u128>` — sum of ORO held across all unbonding entries.
- `ORPHANED_REWARDS: Map<&str, Uint128>` — emissions accrued while
  `TOTAL_STAKES == 0`; owner-recoverable via `ClaimOrphanedRewards`.
- `CONFIG: Item<Config>` — `{ owner, oro_denom, unbonding_period,
  minimum_stake_amount, paused }`.
- `PAUSE_AUTHORITIES: Map<&Addr, ()>` — addresses (besides the owner)
  allowed to pause.
- `OWNERSHIP_PROPOSAL: Item<OwnershipProposal>` — pending two-step owner
  transfer.

Constants (`src/contract.rs`):

- `MAX_UNBONDING_ENTRIES = 10` — per-user.
- `MAX_REWARD_DENOMS = 5` — concurrent active streams.
- `MIN_UNBONDING_PERIOD = 60`, `MAX_UNBONDING_PERIOD = 30 days`
  (enforced in `UpdateConfig`).
- `MAX_REWARD_DURATION = 365 days` (enforced in `AddReward`).

## InstantiateMsg

```json
{
  "owner": "zig1...",
  "oro_denom": "uoro",
  "unbonding_period": 604800,
  "minimum_stake_amount": "1000000"
}
```

- `owner` is taken from the message (not `info.sender`) so factory-style
  deployments can hand off directly to a multisig.
- `oro_denom` is validated to have non-zero on-chain supply.
- `minimum_stake_amount` must be `> 0`.
- No funds are required at instantiate.
- Pause authorities are NOT seeded here — the owner adds them post-deploy
  via `AddPauseAuthorities`.

## ExecuteMsg

- `Enter {}` — deposit ORO via `info.funds` and credit `STAKES[sender] += amount`. Subject to `minimum_stake_amount`.
  ```json
  { "enter": {} }
  ```

- `Leave { amount, recipient }` — debit `amount` from sender's stake and queue an `UnbondingEntry` for `recipient`. Accrued rewards at sync time are attached to the entry and paid at `Claim`. Full exits (`amount == stake`) bypass the minimum; partial exits must meet it.
  ```json
  { "leave": { "amount": "500000", "recipient": "zig1..." } }
  ```

- `Claim {}` — release all matured unbonding entries for `info.sender`. Matured entries are grouped by `receiver` into one `BankMsg::Send` per unique destination (ORO + reward coins merged).
  ```json
  { "claim": {} }
  ```

- `AddReward { duration }` — register/top-up a reward stream. Denom and amount come from `info.funds` (single coin); `rps = amount / duration`. If the denom is already active, the remainder is rolled into a new stream of length `duration`. If re-adding a finished denom, the new stream resumes from its `FINISHED_REWARDS` index. Owner only.
  ```json
  { "add_reward": { "duration": 604800 } }
  ```

- `ClaimOrphanedRewards { recipient? }` — sweep `ORPHANED_REWARDS` to `recipient` (default: owner). Owner only. Errors if no orphaned rewards.
  ```json
  { "claim_orphaned_rewards": { "recipient": "zig1..." } }
  ```

- `UpdateConfig { unbonding_period?, minimum_stake_amount? }` — owner only. `unbonding_period` must be in `[60, 30 days]`; `minimum_stake_amount` must be `> 0`. Ownership transfer is intentionally not exposed here — use the propose/claim flow.
  ```json
  { "update_config": { "unbonding_period": 1209600, "minimum_stake_amount": "2000000" } }
  ```

- `Pause {}` — owner or pause-authority.
- `Unpause {}` — owner only.
- `AddPauseAuthorities { addresses }` / `RemovePauseAuthorities { addresses }` — owner only.
  ```json
  { "add_pause_authorities": { "addresses": ["zig1..."] } }
  ```

- `WithdrawExcessTokens { denom, recipient? }` — sweep a non-ORO, non-reward-stream balance (live or finished) to `recipient` (default: owner). Owner only.
  ```json
  { "withdraw_excess_tokens": { "denom": "uatom" } }
  ```

- `ProposeNewOwner { owner, expires_in }` / `DropOwnershipProposal {}` / `ClaimOwnership {}` — two-step ownership handover (owner proposes, proposed address claims within `expires_in`).

## QueryMsg

- `Config {} -> Config` — full config struct.
- `State {} -> StateResponse { total_shares, total_deposit }` — under the 1:1 model both fields equal `TOTAL_STAKES`.
- `StakeOf { address } -> Uint128` — caller's recorded stake.
- `TotalShares {} -> Uint128` — alias for `TOTAL_STAKES` (legacy field name retained for API stability).
- `PendingRewards { address } -> Vec<Coin>` — read-only mirror of `update_rewards` + `sync_user_rewards`; includes accrual from both live `REWARDS` and `FINISHED_REWARDS`.
- `UnbondingEntries { address } -> Vec<UnbondingEntry>` — full pending queue.
- `UnbondingPeriod {} -> u64` — current `config.unbonding_period` in seconds.
- `TotalLocked {} -> Uint128` — sum of ORO across all unbonding entries.
- `PauseAuthorities {} -> Vec<String>` — registered authorities (owner is NOT included; their authority is implicit).

## Invariants

- `STAKES[user]` only changes via `Enter` (gated by `must_pay`) or `Leave`
  (gated by a typed amount). Rewards never touch `STAKES`.
- `TOTAL_STAKES == sum(STAKES.values())` at all times.
- Contract ORO bank balance `>= sum(STAKES) + TOTAL_LOCKED`. (Excess is
  not credited to any user — direct bank-sends of ORO are stranded.)
- For each reward denom: contract balance of that denom `>=` active stream
  remainder `+ sum(USER_REWARDS.pending_rewards[denom])
  + sum(UnbondingEntry.reward_coins[denom])
  + ORPHANED_REWARDS[denom]`.
- ORO is non-recoverable via `WithdrawExcessTokens` (`OroNonRecoverable`).
- Any denom currently in `REWARDS` or `FINISHED_REWARDS` is non-recoverable
  via `WithdrawExcessTokens` (`DenomInRewardStream`).
- `last_update_ts` advances on every `update_rewards` pass — including the
  `TOTAL_STAKES == 0` orphan branch — so the same window is never emitted
  twice.

## Lifecycle

1. Owner deploys with `minimum_stake_amount = 1_000_000` (1 ORO at 6 decimals)
   and `unbonding_period = 604_800` (7 days).
2. Owner optionally calls `AddPauseAuthorities { addresses }`.
3. Alice calls `Enter {}` with `N uoro` attached. `STAKES[alice] += N`,
   `TOTAL_STAKES += N`.
4. Owner calls `AddReward { duration: 60 }` with `60_000 uzig` attached.
   A stream of `1000 uzig/s` is registered for 60 seconds.
5. After accrual, Alice calls `Leave { amount, recipient: alice }`. Her
   stake is debited; accrued rewards are snapshotted into a new
   `UnbondingEntry { oro_amount: amount, reward_coins, unlock_time: now + 604800, receiver: alice }`,
   and `TOTAL_LOCKED += amount`.
6. After `unbonding_period` elapses, Alice calls `Claim {}`. Matured
   entries are paid out, grouped by `receiver`, in one `BankMsg::Send`
   per unique destination (ORO and reward coins merged).

## Pause Model

- `Pause` is callable by the owner or any address in `PAUSE_AUTHORITIES`.
- `Unpause` is callable by the owner only. Asymmetric by design: a
  compromised pause-authority key can pause to grief but cannot toggle
  the contract back open, so the failure mode is bounded.
- While paused, the following are blocked: `Enter`, `Leave`, `Claim`,
  `ClaimOrphanedRewards`. Administrative paths (`AddReward`,
  `WithdrawExcessTokens`, `UpdateConfig`, pause-authority management,
  ownership rotation) remain available so the owner can react.

## Audit History

- **FIND-006 (2026)** — closed by removing the transferable share token and
  switching to an internal `STAKES` registry. The classic first-depositor /
  share-inflation attack is structurally impossible here: there is no
  share-conversion math, a donation of ORO is not credited to anyone's
  stake, and `Enter` / `Leave` operate strictly 1:1.
- **FIND-015** — closed by lazy-pruning fully-elapsed streams into
  `FINISHED_REWARDS` so the `MAX_REWARD_DENOMS` cap counts only
  concurrent active streams while preserving accrual for late syncers.
