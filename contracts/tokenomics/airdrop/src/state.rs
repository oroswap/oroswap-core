use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use oroswap::common::OwnershipProposal;

#[cw_serde]
pub struct Config {
    pub admin: Addr,
}

#[cw_serde]
pub struct AirdropInfo {
    pub id: u64,
    pub token_address: String,
    pub start_time: u64,
    pub vesting_start_time: u64,
    pub vesting_end_time: u64,
    pub initial_claim_percent: u64,
    pub apy_bonus_bps: u64,
    pub is_active: bool,
    /// true = native/IBC denom, false = CW20 contract.
    pub is_native: bool,
    pub expiry_time: Option<u64>,
}

#[cw_serde]
pub struct UserClaim {
    pub amount_eligible: Uint128,
    pub amount_claimed: Uint128,
    pub claim_time: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LATEST_AIRDROP_ID: Item<u64> = Item::new("latest_airdrop_id");
pub const AIRDROPS: Map<u64, AirdropInfo> = Map::new("airdrops");

/// Secondary index keyed by `(token_address, is_native, airdrop_id)` →
/// `()`. Lets `execute_withdraw_funds` enumerate only the airdrops that
/// use a specific token instead of scanning the entire `AIRDROPS` map,
/// bounding the scan from O(n_global) to O(n_per_token). The `bool`
/// `is_native` is encoded as `u8` (0 = cw20, 1 = native) because
/// `cw-storage-plus` does not implement `PrimaryKey` for `bool`.
///
/// Append-only: an `AirdropInfo`'s `token_address`/`is_native` are fixed
/// at creation, so no toggle/update/claim path needs to touch this map.
pub const AIRDROPS_BY_TOKEN: Map<(&str, u8, u64), ()> = Map::new("airdrops_by_token");
pub const ELIGIBILITY: Map<(u64, &Addr), Uint128> = Map::new("eligibility");
pub const USER_CLAIMS: Map<(u64, &Addr), UserClaim> = Map::new("user_claims");
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// Running total of eligibility amounts per airdrop. Updated on
/// `AddAirdrop` and `UpdateEligibility`. Enables O(1) solvency queries
/// instead of scanning the entire `ELIGIBILITY` map.
pub const ELIGIBILITY_TOTAL: Map<u64, Uint128> = Map::new("eligibility_total");

/// Running total of `amount_claimed` per airdrop. Updated on each `Claim`.
pub const CLAIMED_TOTAL: Map<u64, Uint128> = Map::new("claimed_total");

/// Running total of `amount_eligible` for users who have already claimed,
/// per airdrop. Lets the solvency gate compute the unpaid vesting base
/// (and thus the worst-case APY bonus) from only the remaining unclaimed
/// recipients, instead of inflating it with the eligibility of users
/// whose bonus liability has already been settled.
pub const CLAIMED_ELIGIBILITY_TOTAL: Map<u64, Uint128> = Map::new("claimed_eligibility_total");
