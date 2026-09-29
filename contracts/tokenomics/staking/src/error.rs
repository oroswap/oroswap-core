use cosmwasm_std::{StdError, Uint128};
use cw_utils::{ParseReplyError, PaymentError};
use thiserror::Error;

/// Staking contract errors.
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("{0}")]
    ParseReplyError(#[from] ParseReplyError),

    #[error("Insufficient amount of Stake")]
    StakeAmountTooSmall {},

    #[error("Failed to parse or process reply message")]
    FailedToParseReply {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("Invalid duration")]
    InvalidDuration {},

    #[error("Too many unbonding entries, maximum is {max}")]
    TooManyUnbondingEntries { max: usize },

    #[error("Too many reward denoms, maximum is {max}")]
    TooManyRewardDenoms { max: usize },

    #[error("ORO is non-recoverable: it is the principal asset and every wei held by the contract must remain available to honour Enter/Leave/Claim at the 1:1 stake-registry invariant. Direct bank-sends of ORO are NOT credited to anyone's stake and cannot be swept either — they simply sit in the contract balance.")]
    OroNonRecoverable {},

    #[error("Insufficient stake: requested {requested}, available {available}")]
    InsufficientStake { available: Uint128, requested: Uint128 },

    #[error("Denom `{denom}` is or has been used as a reward stream. The contract owes accrued-but-unclaimed amounts of it to stakers (USER_REWARDS) and to pending UnbondingEntry.reward_coins. Sweeping would brick user Claim calls.")]
    DenomInRewardStream { denom: String },

    #[error("No excess tokens to withdraw")]
    NoExcessToWithdraw {},

    #[error("Invalid pause authority address: {address}")]
    InvalidPauseAuthority { address: String },

    #[error("No unpause authority - only contract owner can unpause")]
    NoUnpauseAuthority {},

    #[error("Amount {provided} below minimum stake amount {minimum}")]
    AmountBelowMinimum { minimum: Uint128, provided: Uint128 },

    #[error("Minimum stake amount must be greater than zero")]
    MinimumStakeAmountIsZero,

    #[error("No orphaned rewards to claim")]
    NoOrphanedRewards,
}
