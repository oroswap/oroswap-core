use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    AddAirdrop {
        token_address: String,
        start_time: u64,
        vesting_start_time: u64,
        vesting_end_time: u64,
        initial_claim_percent: u64,
        apy_bonus_bps: u64,
        eligibility: Vec<(String, Uint128)>,
    },
    ToggleAirdrop {
        id: u64,
        active: bool,
    },
    /// Claim airdrop tokens. One-time settlement.
    Claim {
        airdrop_id: u64,
    },
    DepositFunds {
        token_address: String,
        amount: Uint128,
    },
    WithdrawFunds {
        token_address: String,
        amount: Uint128,
    },
    UpdateConfig {
        admin: Option<String>,
    },
    UpdateEligibility {
        airdrop_id: u64,
        eligibility: Vec<(String, Uint128)>,
    },
}


#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(AirdropResponse)]
    Airdrop { id: u64 },
    #[returns(LatestAirdropIdResponse)]
    LatestAirdropId {},
    #[returns(UserAirdropStatusResponse)]
    UserAirdropStatus { id: u64, address: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: Addr,
}

#[cw_serde]
pub struct AirdropResponse {
    pub id: u64,
    pub token_address: String,
    pub start_time: u64,
    pub vesting_start_time: u64,
    pub vesting_end_time: u64,
    pub initial_claim_percent: u64,
    pub apy_bonus_bps: u64,
    pub is_active: bool,
}



#[cw_serde]
pub struct LatestAirdropIdResponse {
    pub id: u64,
}

#[cw_serde]
pub struct UserAirdropStatusResponse {
    /// Total tokens allocated to this user (from ELIGIBILITY)
    pub total_allocation: Uint128,
    /// Amount claimable right now (vesting-aware calculation)
    pub current_claimable: Uint128,
    /// Claim status
    pub is_claimed: bool,
    pub amount_claimed: Uint128,
    pub claim_time: Option<u64>,
    /// Vesting phase info for UX: "not_started" | "in_progress" | "completed"
    pub vesting_status: String,
}
