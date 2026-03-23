use cosmwasm_std::StdError;
use cw_utils::{ParseReplyError, PaymentError};
use thiserror::Error;

/// This enum describes wrapper contract errors
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("{0}")]
    ParseReplyError(#[from] ParseReplyError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("Failed to parse or process reply message")]
    FailedToParseReply {},

    #[error("Amount must be greater than zero")]
    ZeroAmount {},

    #[error("Contract can't be migrated!")]
    MigrationError {},
}
