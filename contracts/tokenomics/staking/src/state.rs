use cw_storage_plus::{Item, Map};
use cosmwasm_std::Addr;

use oroswap::staking::{Config, TrackerData, UnbondingEntry};

/// Stores the contract config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Total ORO locked in pending unbonding entries.
/// This is subtracted from the pool balance when calculating the xORO ratio
/// to ensure unbonding ORO doesn't dilute rewards for active stakers.
pub const TOTAL_LOCKED: Item<u128> = Item::new("total_locked");

/// Per-user unbonding queue. Each entry stores the fixed ORO amount and unlock timestamp.
pub const UNBONDING: Map<&Addr, Vec<UnbondingEntry>> = Map::new("unbonding");
