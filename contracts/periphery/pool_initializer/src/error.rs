use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Invalid reply ID")]
    InvalidReplyId {},

    #[error("Failed to query factory for pair info")]
    FailedToQueryFactory {},

    #[error("No pending liquidity found")]
    NoPendingLiquidity {},

    #[error("Invalid factory address")]
    InvalidFactoryAddress {},

    #[error("Invalid receiver address")]
    InvalidReceiverAddress {},

    #[error("Invalid asset info")]
    InvalidAssetInfo {},

    #[error("Invalid pair type")]
    InvalidPairType {},

    #[error("Invalid slippage tolerance")]
    InvalidSlippageTolerance {},

    #[error("Invalid initial liquidity")]
    InvalidInitialLiquidity {},

    #[error("Insufficient funds for pool creation fee")]
    InsufficientFunds {},

    #[error("Unauthorized")]
    Unauthorized {},


}
