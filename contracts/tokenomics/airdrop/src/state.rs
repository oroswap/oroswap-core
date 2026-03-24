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
    /// true = native/IBC denom, false = CW20 contract (C-1 fix)
    pub is_native: bool,
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
pub const ELIGIBILITY: Map<(u64, &Addr), Uint128> = Map::new("eligibility");
pub const USER_CLAIMS: Map<(u64, &Addr), UserClaim> = Map::new("user_claims");
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
