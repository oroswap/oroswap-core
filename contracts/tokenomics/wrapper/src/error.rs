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

    #[error("No pause authority")]
    NoPauseAuthority {},

    #[error("Invalid pause authority address: {address}")]
    InvalidPauseAuthority { address: String },

    #[error("No unpause authority - only contract owner can unpause")]
    NoUnpauseAuthority {},

    #[error("mint_max_supply must be greater than zero (set it to the source asset's max supply in base units)")]
    InvalidMintMaxSupply {},

    #[error("Lock would push circulating supply over the configured cap. current_supply={current}, lock_amount={mint}, max_supply={cap}")]
    MaxSupplyExceeded {
        current: cosmwasm_std::Uint128,
        mint: cosmwasm_std::Uint128,
        cap: cosmwasm_std::Uint128,
    },
}
