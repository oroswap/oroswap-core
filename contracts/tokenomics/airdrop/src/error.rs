use cosmwasm_std::{StdError, Uint128};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Airdrop not found")]
    AirdropNotFound {},

    #[error("Not eligible for this airdrop")]
    NotEligible {},

    #[error("Airdrop already claimed")]
    AlreadyClaimed {},

    #[error("Airdrop has not started yet")]
    AirdropNotStarted {},

    #[error("Airdrop is currently inactive")]
    AirdropInactive {},

    #[error("Airdrop already started")]
    AirdropAlreadyStarted {},

    #[error("Airdrop has expired")]
    AirdropExpired {},

    #[error("Contract name mismatch during migration")]
    MigrationError {},

    #[error("Invalid vesting window: require start_time <= vesting_start_time < vesting_end_time")]
    InvalidVestingWindow {},

    #[error("Invalid initial_claim_percent: must be <= 100, got {got}")]
    InvalidInitialPercent { got: u64 },

    #[error("start_time must be >= current block time")]
    StartTimeInPast {},

    #[error("Invalid expiry_time: must be > vesting_end_time")]
    InvalidExpiry {},

    #[error("apy_bonus_bps too high: max {max}, got {got}")]
    ApyBonusTooHigh { got: u64, max: u64 },

    #[error("Eligibility batch too large: got {got}, max {max}. Split the input into multiple UpdateEligibility calls.")]
    EligibilityBatchTooLarge { got: usize, max: usize },

    #[error("Withdrawal blocked: airdrop {airdrop_id} is still active or has not reached its expiry. Toggle it off and wait until expiry_time before withdrawing.")]
    WithdrawalBlocked { airdrop_id: u64 },

    #[error("Withdrawal blocked: airdrop {airdrop_id} uses this token and has no expiry_time configured (cannot be reclaimed).")]
    WithdrawalBlockedNoExpiry { airdrop_id: u64 },

    #[error("APY decrease not allowed once the airdrop has started: old {old} bps, attempted {new} bps")]
    ApyDecreaseNotAllowed { old: u64, new: u64 },

    #[error("APY update blocked: airdrop has reached vesting_end_time — bonus value is locked")]
    ApyUpdateAfterVestingEnd {},

    #[error("APY update would exceed contract funding: max_outstanding {required} > balance {available}. Deposit funds first.")]
    ApyUpdateUnderfunded { required: Uint128, available: Uint128 },
}
