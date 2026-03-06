use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub admin: Addr,
}

#[cw_serde]
pub struct AirdropInfo {
    pub token_address: String,
    pub start_time: u64,
    pub end_time: u64,
    pub vesting_duration: u64,
    pub initial_claim_percent: u64,
    pub apy_bonus_bps: u64,
}

#[cw_serde]
pub struct UserClaim {
    pub amount_eligible: Uint128,
    pub initial_claimed: Uint128,
    pub vesting_amount: Uint128,
    pub start_time: u64,
    pub last_withdrawal_time: u64,
    pub withdrawn_amount: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LATEST_AIRDROP_ID: Item<u64> = Item::new("latest_airdrop_id");
pub const AIRDROPS: Map<u64, AirdropInfo> = Map::new("airdrops");
pub const ELIGIBILITY: Map<(u64, &Addr), Uint128> = Map::new("eligibility");
pub const USER_CLAIMS: Map<(u64, &Addr), UserClaim> = Map::new("user_claims");
