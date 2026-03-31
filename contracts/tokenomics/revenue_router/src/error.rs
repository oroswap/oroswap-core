use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Zero USDC balance")]
    ZeroBalance {},

    #[error("Insufficient balance: requested {requested}, available {balance}")]
    InsufficientBalance { requested: String, balance: String },

    #[error("Migration error")]
    MigrationError {},

    #[error("Invalid IBC timeout configuration")]
    InvalidTimeout {},

    #[error("Cannot change usdc_denom while contract holds balance of old denom")]
    StrandedBalance {},

    #[error("Contract is paused")]
    Paused {},
}
