#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response};
use cw2::{get_contract_version, set_contract_version};

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::state::{REWARD_ORO_RESERVE, TOTAL_LOCKED};
use oroswap::staking::MigrateMsg;
use cosmwasm_std::Uint128;

/// Migration entry point.
///
/// This contract is freshly deployed — no legacy state to migrate.
/// The handler initialises any state items that may be absent on older
/// deployments and bumps the stored contract version.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;

    // Only allow migrating from the same contract name.
    if contract_version.contract != CONTRACT_NAME {
        return Err(ContractError::MigrationError {});
    }

    // Ensure new state items exist (idempotent — safe to run on any version).
    if REWARD_ORO_RESERVE.may_load(deps.storage)?.is_none() {
        REWARD_ORO_RESERVE.save(deps.storage, &Uint128::zero())?;
    }
    if TOTAL_LOCKED.may_load(deps.storage)?.is_none() {
        TOTAL_LOCKED.save(deps.storage, &0u128)?;
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}
