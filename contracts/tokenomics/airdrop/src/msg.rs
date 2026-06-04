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
        /// Required. Self-flagged follow-up to FIND-008/FIND-001 audit cycle:
        /// previously optional, but `None` had no admin escape — leftover
        /// tokens locked permanently. Now mandatory and validated to be
        /// strictly greater than `vesting_end_time` so `WithdrawFunds` always
        /// has a deterministic reclaim window.
        expiry_time: u64,
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
    /// Admin-only. Update the APY bonus rate for a season. Two-phase gate:
    ///   - Before `start_time`: any direction allowed (configuration fix).
    ///   - `start_time <= now < vesting_end_time`: strictly increase only —
    ///     decreases would punish users who already chose to wait based on
    ///     the advertised APY.
    ///   - `now >= vesting_end_time`: rejected — some users may have already
    ///     claimed with the old value; the rate must stay consistent.
    /// New value is capped at `MAX_APY_BONUS_BPS` and any increase is
    /// rejected if the contract balance cannot cover the resulting
    /// worst-case payout (`max_outstanding`).
    UpdateApyBonus {
        airdrop_id: u64,
        apy_bonus_bps: u64,
    },
    /// Admin-only. Push `start_time`, `vesting_start_time`, and/or
    /// `vesting_end_time` forward before the airdrop begins. Each supplied
    /// field must be **strictly greater** than the currently stored value
    /// (forward-only — no shortening of the launch window or vesting). All
    /// existing invariants are re-checked against the resulting state:
    ///   - `now < airdrop.start_time` (admin can only update pre-launch).
    ///   - At least one field must be `Some`.
    ///   - `new_start_time >= now` (no past).
    ///   - `new_start_time <= new_vesting_start_time < new_vesting_end_time`.
    ///   - `airdrop.expiry_time > new_vesting_end_time` (extend expiry first
    ///     via `UpdateExpiry` if needed).
    /// Unsupplied fields fall back to the currently stored value.
    UpdateTimes {
        airdrop_id: u64,
        start_time: Option<u64>,
        vesting_start_time: Option<u64>,
        vesting_end_time: Option<u64>,
    },
    /// Admin-only. Extend `expiry_time` further into the future. The new
    /// value must be strictly greater than the currently stored expiry
    /// (forward-only — admin cannot bring the reclaim window forward and
    /// short-change unclaimed users). Permitted at any phase BEFORE the
    /// current expiry has passed:
    ///   - `now < airdrop.expiry_time` (can't extend a passed expiry).
    ///   - `expiry_time > airdrop.expiry_time` (forward-only).
    ///   - `expiry_time > airdrop.vesting_end_time` (invariant preserved).
    UpdateExpiry {
        airdrop_id: u64,
        expiry_time: u64,
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
    pub expiry_time: u64,
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
    /// Σ `amount_eligible` for users who have NOT yet claimed. Pessimistic
    /// "still-to-pay if every remaining user claims at vesting end" bound,
    /// before the APY bonus. Forfeited mid-vesting portions are excluded —
    /// once a user claims, their entire eligibility is settled.
    pub outstanding: Uint128,
    /// Worst-case outstanding: assumes every unclaimed user claims at or
    /// after `vesting_end_time` and receives the full APY bonus. The bonus
    /// base is the remaining (unclaimed) vesting portion only.
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
