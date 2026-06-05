//! Staking contract state — 1:1 stake registry + per-denom reward accumulator.
use cw_storage_plus::{Item, Map};
use cosmwasm_std::{Addr, Decimal256, Uint128};
use cosmwasm_schema::cw_serde;

use oroswap::common::OwnershipProposal;
use oroswap::staking::{Config, UnbondingEntry};

#[cw_serde]
pub struct RewardInfo {
    pub denom: String,
    pub rps: Decimal256,
    pub index: Decimal256,
    pub last_update_ts: u64,
    pub end_ts: u64,
}

#[cw_serde]
pub struct UserRewardInfo {
    pub last_rewards_index: Vec<(String, Decimal256)>,
    /// Exact fractional reward balance per denom. Flooring happens only at
    /// the boundary where Decimal256 leaves the contract (Leave → UnbondingEntry
    /// snapshot, PendingRewards query → UI). Carrying the sub-unit remainder
    /// across syncs eliminates per-interaction dust loss: a user's lifetime
    /// payout equals their proportional share down to at most one final floor
    /// at full exit.
    pub pending_rewards: Vec<(String, Decimal256)>,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// Per-user ORO stake balance. Updated on Enter / Leave by exactly the ORO
/// amount supplied — no share conversion.
pub const STAKES: Map<&Addr, Uint128> = Map::new("stakes");

/// Sum of all entries in `STAKES`. Equal to the total ORO principal backing
/// live stakers (excluding unbonding and reward float).
pub const TOTAL_STAKES: Item<Uint128> = Item::new("total_stakes");

/// Total ORO locked in pending unbonding entries. Tracked separately from
/// `TOTAL_STAKES` so excess-token sweeps and solvency checks can reason
/// about it explicitly.
pub const TOTAL_LOCKED: Item<u128> = Item::new("total_locked");

/// Per-user unbonding queue.
pub const UNBONDING: Map<&Addr, Vec<UnbondingEntry>> = Map::new("unbonding");

/// Active reward streams.
pub const REWARDS: Item<Vec<RewardInfo>> = Item::new("rewards");

/// FIND-015: frozen final index per denom for reward streams that have fully
/// elapsed AND been pruned out of the active `REWARDS` list. Lets
/// `MAX_REWARD_DENOMS` cap *concurrent active* streams instead of *lifetime*
/// denoms.
///
/// Maintenance:
///   - Written in `update_rewards` when `end_ts <= block_ts` (after accrual
///     has been capped at `end_ts`).
///   - Read in `sync_user_rewards` / `PendingRewards` as a fallback for any
///     denom in a user's snapshot no longer in the live list — credits them
///     `(finished_index - snapshot) * shares`.
///   - Removed in `execute_add_reward` when the admin re-adds a finished
///     denom; the new RewardInfo resumes from this index so live-loop
///     accrual stays continuous.
///
/// An entry here is still a liability: `execute_withdraw_excess_tokens`
/// refuses to sweep balance of any denom present in this map.
pub const FINISHED_REWARDS: Map<&str, Decimal256> = Map::new("finished_rewards");

/// Per-user reward tracking information.
pub const USER_REWARDS: Map<&Addr, UserRewardInfo> = Map::new("user_rewards");

/// Orphaned rewards keyed by denom. Populated by `update_rewards` when time
/// advances while `TOTAL_STAKES == 0` so emissions are recoverable via
/// `ClaimOrphanedRewards` instead of silently dropped. Accrual is floored to
/// Uint128 at orphan time (up to N-1 wei may be lost per orphan event across
/// N concurrent denoms). `last_update_ts` MUST still advance through the
/// orphan branch or the same window would be re-emitted when stakers arrive.
pub const ORPHANED_REWARDS: Map<&str, Uint128> = Map::new("orphaned_rewards");

/// Pending ownership proposal created via ProposeNewOwner.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// Addresses (in addition to the owner) authorised to pause the contract.
/// Only the owner can add/remove entries and only the owner can unpause, so
/// a compromised authority can pause but cannot grief by toggling.
pub const PAUSE_AUTHORITIES: Map<&Addr, ()> = Map::new("pause_authorities");
