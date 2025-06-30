use oroswap::asset::AssetInfo;
use cosmwasm_std::{DivideByZeroError, OverflowError, StdError};
use thiserror::Error;

/// This enum describes maker contract errors
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid bridge {0} to {1}")]
    InvalidBridge(AssetInfo, AssetInfo),

    #[error("Invalid bridge. Pool {0} to {1} not found")]
    InvalidBridgeNoPool(String, String),

    #[error("Invalid bridge destination: {0}")]
    InvalidBridgeDestination(String),

    #[error("Max bridge depth reached: {0}")]
    MaxBridgeDepth(u64),

    #[error("Cannot swap {0}. No swap destinations")]
    CannotSwap(AssetInfo),

    #[error("Incorrect governance percent of its share")]
    IncorrectGovernancePercent {},

    #[error("Governance percent must be 100% when staking contract is not set")]
    GovernancePercentMustBe100 {},

    #[error("Incorrect max spread")]
    IncorrectMaxSpread {},

    #[error("Cannot collect. Remove duplicate asset")]
    DuplicatedAsset {},

    #[error("Rewards collecting is already enabled")]
    RewardsAlreadyEnabled {},

    #[error("An error occurred during migration")]
    MigrationError {},

    #[error("Collect cooldown is not expired. Next collect is possible at {next_collect_ts}")]
    Cooldown { next_collect_ts: u64 },

    #[error("Incorrect cooldown. Should be between {min} and {max}")]
    IncorrectCooldown { min: u64, max: u64 },

    #[error("Pool not found")]
    PoolNotFound {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}

impl From<DivideByZeroError> for ContractError {
    fn from(err: DivideByZeroError) -> Self {
        StdError::from(err).into()
    }
}
