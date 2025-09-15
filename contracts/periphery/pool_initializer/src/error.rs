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

    #[error("Asset mismatch between asset_infos and liquidity.assets")]
    AssetMismatch {},

    #[error("Insufficient funds for pool creation fee")]
    InsufficientFunds {},

    #[error("Insufficient funds for denom: {denom}")]
    InsufficientFundsForDenom { denom: String },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Failed to parse reply")]
    FailedToParseReply {},

    #[error("Factory creation failed: {error}")]
    FactoryCreationFailed { error: String },

    #[error("User already has an operation in progress")]
    OperationInProgress {},

    #[error("Insufficient CW-20 allowance for token {token}: required {required}, current {current}")]
    InsufficientCw20Allowance { token: String, required: cosmwasm_std::Uint128, current: cosmwasm_std::Uint128 },
}
