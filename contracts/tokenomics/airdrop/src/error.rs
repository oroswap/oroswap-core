use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid time: start_time must be before end_time")]
    InvalidTime {},

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
}
