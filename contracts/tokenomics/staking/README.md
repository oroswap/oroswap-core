# Oroswap xORO Staking (Native)

Share-based ORO vault: users deposit ORO and receive a tokenfactory-denomi-
nated share token (`xORO`, full denom `coin.<contract>.xoro`). The xORO:ORO
exchange rate grows monotonically as the pool earns rewards, so all share-
holders auto-compound without an explicit claim action. A separate admin-
funded reward stream (`AddReward`) can pay out any denom (including ORO) on
a linear schedule, settled alongside the principal on `Claim`.

## Features

- **Native Token Staking** — fully native; `uzig` / tokenfactory ORO compatible.
- **Auto-Compounding** — rewards that flow into the pool inflate every xORO share's redemption value simultaneously, no per-user claim needed for that leg.
- **Multi-denom Reward Streams** — admin can run up to `MAX_REWARD_DENOMS = 5` simultaneous reward streams via `AddReward { duration }`. Each stream is linearly distributed pro-rata across xORO holders and paid out on `Claim`.
- **Unbonding Period** — `Leave` queues an `UnbondingEntry`; principal + accrued rewards become claimable after the global `unbonding_period`. Per-user cap: `MAX_UNBONDING_ENTRIES = 10` concurrent entries.
- **Pause / Unpause** — owner OR any delegated pause-authority can halt Enter / Leave / Claim instantly. **Only the owner can unpause** (compromise-resistant; mirrors the factory + wrapper pattern).
- **Two-step Ownership Transfer** — standard `ProposeNewOwner` → `ClaimOwnership` (with `DropOwnershipProposal` cancellation).
- **TokenFactory Integration** — automatically creates the `xoro` denom and manages mint/burn via Zigchain's factory module.
- **Excess-Token Recovery** — owner can sweep non-ORO tokens accidentally bank-sent to the contract via `WithdrawExcessTokens`.

---

## InstantiateMsg

```json
{
  "deposit_token_denom": "coin.zig1.../oro",
  "bootstrap_amount": "1000",
  "unbonding_period": 604800,
  "xoro_decimals": 6,
  "xoro_uri": "https://example.com/xoro.json"
}
```

| Field | Type | Notes |
|---|---|---|
| `deposit_token_denom` | `String` | The ORO denom users will stake. Must have non-zero supply on chain at instantiate. |
| `bootstrap_amount` | `Option<Uint128>` | Initial xORO minted to the contract to prevent first-deposit share-inflation attacks. Defaults to and must be ≥ `MINIMUM_STAKE_AMOUNT = 1_000`. Funds matching this amount must be sent with the instantiate tx. |
| `unbonding_period` | `u64` | Seconds the user must wait between `Leave` and `Claim`. Global, owner-updatable. |
| `xoro_decimals` | `Option<u32>` | Display decimals on the `xORO` tokenfactory metadata. Default 6. |
| `xoro_uri` | `Option<String>` | Optional URI for xORO metadata. |

The contract creates `coin.<contract_address>.xoro` via Zigchain's factory module, mints the bootstrap supply to itself, and (if `zigchain` feature is enabled) sets denom metadata. The owner is the instantiate sender.

## ExecuteMsg

### `Enter { receiver }`
Deposit ORO and receive xORO at the current exchange rate. Send the ORO as funds with the transaction. `receiver` defaults to the sender if omitted.

```json
{ "enter": { "receiver": "zig1..." } }
```

Mint amount: `xoro_minted = oro_in × total_shares / effective_deposit`, where `effective_deposit = oro_balance − total_locked − reward_oro_reserve`.

### `EnterWithHook { contract_address, msg }`
Like `Enter`, but the resulting xORO is forwarded **in full** to `contract_address` along with the supplied execute `msg`. Used for atomically depositing-into / re-depositing-from other protocols.

```json
{
  "enter_with_hook": {
    "contract_address": "zig1someprotocol...",
    "msg": "eyJzb21lX2FjdGlvbiI6e319"
  }
}
```

### `Leave { receiver }`
Burn xORO and queue an unbonding entry. Send xORO as funds with the tx. `receiver` is locked in at `Leave` time and is who will receive the funds on `Claim`. Defaults to the sender.

```json
{ "leave": { "receiver": "zig1..." } }
```

Side effects: burns the xORO immediately, decrements `effective_deposit`, increments `total_locked`, snapshots any accrued reward-stream rewards into the entry's `reward_coins`. Capped at `MAX_UNBONDING_ENTRIES = 10` per user.

### `Claim`
Sweep all matured unbonding entries (those past `unlock_time`) for the sender. Sends ORO principal + matured reward-coin payouts to each entry's stored `receiver`.

```json
{ "claim": {} }
```

### `AddReward { duration }`
Admin only. Open or top up a reward stream. Send the reward coin (any denom, including ORO) with the tx; the contract registers a linear distribution over `duration` seconds, pro-rata across xORO holders. Up to `MAX_REWARD_DENOMS = 5` distinct denoms simultaneously.

```json
{ "add_reward": { "duration": 86400 } }
```

When the reward coin **is** the ORO denom, the amount is held in `REWARD_ORO_RESERVE` so it doesn't artificially inflate `effective_deposit` (which would otherwise mis-credit it to existing stakers' share value before it's streamed out).

### `Pause` / `Unpause`
- `Pause` is callable by the owner OR any address in `PAUSE_AUTHORITIES` (see below).
- `Unpause` is **owner only** — a compromised pause-authority key can grief by pausing but cannot toggle pause state back and forth.

Halts (resumes) all `Enter`, `EnterWithHook`, `Leave`, and `Claim` calls. Reward streams continue to accrue in storage — they just can't be settled while paused.

```json
{ "pause": {} }
{ "unpause": {} }
```

### `AddPauseAuthorities` / `RemovePauseAuthorities`
Owner only. Manages the set of additional addresses authorised to pause the contract. Mirrors the factory + wrapper pattern. Authorities can `Pause` but cannot `Unpause`.

```json
{ "add_pause_authorities":    { "authorities": ["zig1ops...", "zig1bot..."] } }
{ "remove_pause_authorities": { "authorities": ["zig1bot..."] } }
```

### `UpdateConfig { unbonding_period }`
Owner only. Currently the only mutable config field. Other fields (denoms, decimals) are immutable for the contract's lifetime.

```json
{ "update_config": { "unbonding_period": 1209600 } }
```

### `WithdrawExcessTokens { denom, recipient }`
Owner only. Sweeps the contract's full balance of a non-ORO `denom` to `recipient` (defaults to owner). Intended for mistakenly bank-sent tokens.

**ORO is explicitly non-recoverable** — every ORO held by the contract is owed to stakers (effective deposit), unbonders (`total_locked`), or future reward claimants (`reward_oro_reserve`). Direct bank-sends of ORO auto-compound into existing xORO share value by design; the contract returns `OroNonRecoverable` rather than ship a footgun. (Audit N-1.)

```json
{ "withdraw_excess_tokens": { "denom": "stray/usdc", "recipient": "zig1..." } }
```

### Ownership transfer (two-step)
```json
{ "propose_new_owner": { "owner": "zig1new...", "expires_in": 86400 } }
{ "claim_ownership": {} }
{ "drop_ownership_proposal": {} }
```

## QueryMsg

| Query | Returns | Notes |
|---|---|---|
| `State {}` | `StateResponse { total_shares, total_deposit, exchange_rate }` | Snapshot of pool health. `total_deposit` is `effective_deposit` (ORO backing the live xORO supply). |
| `Config {}` | `Config { oro_denom, xoro_denom, bootstrap_amount, owner, unbonding_period, xoro_decimals, xoro_uri, paused }` | |
| `TotalShares {}` | `Uint128` | Current xORO total supply (queries the bank module). |
| `TotalDeposit {}` | `Uint128` | `effective_deposit` — ORO backing share value. |
| `TotalLocked {}` | `Uint128` | ORO currently queued for unbonding. |
| `BalanceAt { address, timestamp }` | `Uint128` | xORO balance of `address` at `timestamp` (current if omitted). For governance / off-chain snapshots. |
| `TotalSupplyAt { timestamp }` | `Uint128` | xORO total supply at `timestamp`. |
| `UnbondingEntries { address }` | `Vec<UnbondingEntry>` | All pending unbonding entries for `address`. |
| `UnbondingPeriod {}` | `u64` | Global unbonding period in seconds. |
| `PendingRewards { address }` | `Vec<Coin>` | Reward-stream payouts that would settle for `address` if `Leave` were called now. |
| `PauseAuthorities {}` | `Vec<String>` | Addresses with delegated pause authority. Owner has implicit authority and is NOT included. |

## Reward accounting nuances

### Auto-compound vs. side-rewards
Two reward mechanics run side-by-side:

1. **Auto-compound (ORO only)** — any ORO that flows into the pool (admin send, fee-converter output, fee accrual) raises `effective_deposit`. Existing xORO holders' shares immediately redeem for more ORO. No claim action.
2. **Side-reward streams (`AddReward`)** — admin-initiated linear distributions of any denom over a fixed duration. Tracked per-stream in `REWARDS`, per-user accrual recorded in `USER_REWARDS`, settled on `Claim` via the matured `UnbondingEntry.reward_coins` field.

When the side-reward denom **is** ORO, the funded amount sits in `REWARD_ORO_RESERVE` and is excluded from `effective_deposit` until streamed out — preventing the same wei from rewarding both share-value-appreciation and side-stream-claimants.

### Bootstrap-share reward dust (known limitation, audit I-1 / N-4)
The instantiate path mints `bootstrap_amount` xORO **to the contract address itself** to defeat the first-deposit share-inflation attack. Those bootstrap shares are part of `total_shares` (the chain-queried supply that `update_rewards` divides by), but the contract has no user record to call `sync_user_rewards` against. The bootstrap shares' pro-rata cut of every side-reward stream therefore accrues into the global `index` and is never paid out.

**Magnitude:** `dust_per_stream ≈ funded_amount × bootstrap_amount / total_shares_at_time_of_stream`. At the default `bootstrap_amount = 1_000` and any realistic post-launch `total_shares`, the dust per stream is negligible (sub-basis-point). It grows with both `bootstrap_amount` and the number of `AddReward` calls.

**Treatment:** explicitly documented and accepted, not fixed. The reward-eligible supply could be reduced to `total_shares − bootstrap_amount` in `update_rewards`, but doing so changes the accounting invariants that downstream queries (`TotalShares`, `TotalSupplyAt`) rely on and isn't worth the surface area at current operational scale.

---

## Migration

`cw2` same-name guard. The `migrate` handler is idempotent: initialises `REWARD_ORO_RESERVE` and `TOTAL_LOCKED` if absent (older deployments), bumps the cw2 version, and leaves all other state untouched. `Config.paused` is `#[serde(default)]` so legacy bytes without that field deserialise as `false` on first `CONFIG.load()` after migrate. (Audit N-2.)

## Constants

| Constant | Value | Purpose |
|---|---|---|
| `MINIMUM_STAKE_AMOUNT` | 1_000 | Minimum bootstrap amount; prevents first-deposit share-inflation attack. |
| `MAX_UNBONDING_ENTRIES` | 10 | Per-user concurrent unbonding entries; bounds storage for `Claim` iteration. |
| `MAX_REWARD_DENOMS` | 5 | Cap on simultaneously-active side-reward streams. |
