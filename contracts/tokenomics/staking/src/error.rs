use cosmwasm_std::StdError;
use cw_utils::{ParseReplyError, PaymentError};
use thiserror::Error;

/// This enum describes staking contract errors
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

    #[error("Contract can't be migrated!")]
    MigrationError {},

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

    #[error("ORO is non-recoverable: every wei held by the contract is owed to stakers (effective deposit, unbonding queue, or reward reserve). Direct bank-sends of ORO auto-compound into existing xORO share value by design and cannot be reclaimed.")]
    OroNonRecoverable {},

    #[error("No excess tokens to withdraw")]
    NoExcessToWithdraw {},

    #[error("Invalid pause authority address: {address}")]
    InvalidPauseAuthority { address: String },

    #[error("No unpause authority - only contract owner can unpause")]
    NoUnpauseAuthority {},
}
