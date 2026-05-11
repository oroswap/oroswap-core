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
        /// true = native/IBC denom, false = CW20 contract
        is_native: bool,
        start_time: u64,
        vesting_start_time: u64,
        vesting_end_time: u64,
        initial_claim_percent: u64,
        apy_bonus_bps: u64,
        eligibility: Vec<(String, Uint128)>,
        expiry_time: Option<u64>,
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
        is_native: bool,
    },
    WithdrawFunds {
        token_address: String,
        amount: Uint128,
        is_native: bool,
    },
    /// Proposes a new admin. The proposed admin must call ClaimOwnership.
    ProposeNewOwner { owner: String, expires_in: u64 },
    /// Removes the pending ownership proposal.
    DropOwnershipProposal {},
    /// Claims a pending ownership proposal. Only callable by the proposed new admin.
    ClaimOwnership {},
    UpdateEligibility {
        airdrop_id: u64,
        eligibility: Vec<(String, Uint128)>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

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
    /// Solvency snapshot for the given airdrop:
    ///   - Σ eligibility committed to users
    ///   - Σ already claimed
    ///   - the current contract balance of the airdrop's token
    ///   - whether the contract holds enough to cover the worst-case payout
    ///     (all unclaimed users claim after vesting end and receive the APY bonus)
    #[returns(SolvencyResponse)]
    Solvency { id: u64 },
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
    pub is_native: bool,
    pub expiry_time: Option<u64>,
}



#[cw_serde]
pub struct LatestAirdropIdResponse {
    pub id: u64,
}

#[cw_serde]
pub struct SolvencyResponse {
    pub airdrop_id: u64,
    pub token_address: String,
    pub is_native: bool,
    /// Σ amounts assigned via `AddAirdrop` / `UpdateEligibility`.
    pub total_eligibility: Uint128,
    /// Σ amounts actually paid out across all `Claim` calls so far.
    pub total_claimed: Uint128,
    /// `total_eligibility - total_claimed`. Note: forfeited mid-vesting
    /// portions reduce *paid* but not *eligibility*, so this is the
    /// pessimistic "still-to-pay if everyone claims at vesting end" bound,
    /// before APY bonus.
    pub outstanding: Uint128,
    /// Worst-case outstanding: assumes every unclaimed user claims at or
    /// after `vesting_end_time` and receives the full APY bonus.
    pub max_outstanding: Uint128,
    /// Current contract balance for `token_address` (native via bank,
    /// CW20 via the contract's `Balance` query).
    pub contract_balance: Uint128,
    /// True iff `contract_balance >= max_outstanding`.
    pub is_fully_funded: bool,
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
