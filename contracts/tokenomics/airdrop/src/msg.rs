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
        end_time: u64,
        vesting_duration: u64,
        initial_claim_percent: u64,
        apy_bonus_bps: u64,
        eligibility: Vec<(String, Uint128)>,
    },
    Claim {
        airdrop_id: u64,
        choice: ClaimChoice,
    },
    WithdrawVested {
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
}

#[cw_serde]
pub enum ClaimChoice {
    InitialPortion,
    FullVesting,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(AirdropResponse)]
    Airdrop { id: u64 },
    #[returns(EligibilityResponse)]
    Eligibility { id: u64, address: String },
    #[returns(UserClaimResponse)]
    ClaimStatus { id: u64, address: String },
    #[returns(LatestAirdropIdResponse)]
    LatestAirdropId {},
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
    pub end_time: u64,
    pub vesting_duration: u64,
    pub initial_claim_percent: u64,
    pub apy_bonus_bps: u64,
}

#[cw_serde]
pub struct EligibilityResponse {
    pub amount: Uint128,
}

#[cw_serde]
pub struct UserClaimResponse {
    pub claim: Option<UserClaimInfo>,
}

#[cw_serde]
pub struct UserClaimInfo {
    pub amount_eligible: Uint128,
    pub initial_claimed: Uint128,
    pub vesting_amount: Uint128,
    pub start_time: u64,
    pub last_withdrawal_time: u64,
    pub withdrawn_amount: Uint128,
}

#[cw_serde]
pub struct LatestAirdropIdResponse {
    pub id: u64,
}
