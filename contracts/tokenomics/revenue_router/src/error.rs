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
}
