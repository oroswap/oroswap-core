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
    pub pending_rewards: Vec<cosmwasm_std::Coin>,
}

/// Stores the contract config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Total ORO locked in pending unbonding entries.
/// This is subtracted from the pool balance when calculating the xORO ratio
/// to ensure unbonding ORO doesn't dilute rewards for active stakers.
pub const TOTAL_LOCKED: Item<u128> = Item::new("total_locked");

/// Per-user unbonding queue. Each entry stores the fixed ORO amount and unlock timestamp.
pub const UNBONDING: Map<&Addr, Vec<UnbondingEntry>> = Map::new("unbonding");

/// Global reward information for the staking pool.
pub const REWARDS: Item<Vec<RewardInfo>> = Item::new("rewards");

/// Per-user reward tracking information.
pub const USER_REWARDS: Map<&Addr, UserRewardInfo> = Map::new("user_rewards");

pub const REWARD_ORO_RESERVE: Item<Uint128> = Item::new("reward_oro_reserve");

/// Pending ownership proposal created via ProposeNewOwner.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// Addresses (in addition to the owner) authorised to pause the contract.
/// Only the owner can add / remove entries, and only the owner can unpause —
/// so a compromised authority can pause but cannot grief by toggling.
/// Mirrors the wrapper + factory pattern.
pub const PAUSE_AUTHORITIES: Map<&Addr, ()> = Map::new("pause_authorities");
