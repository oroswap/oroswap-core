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

    #[error("Invalid forward_memo: must be a non-empty JSON object")]
    InvalidMemo {},

    #[error("Invalid first_hop_receiver: must be non-empty")]
    InvalidReceiver {},

    #[error("Invalid bridge cap: must be greater than zero (use SetPaused to halt)")]
    InvalidBridgeCap {},

    #[error("Bridge amount {requested} exceeds per-tx cap {cap}")]
    BridgeCapExceeded { requested: String, cap: String },
}
