#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{DepsMut, Env, Response, Uint128};
use cw2::{get_contract_version, set_contract_version};
use cw_storage_plus::Item;

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use oroswap::pair::MigrateMsg;
use oroswap::staking::Config;

use crate::error::ContractError;
use crate::state::CONFIG;

#[cw_serde]
pub struct ConfigV2_1_0 {
    pub oro_denom: String,
    pub xoro_denom: String,
    pub bootstrap_amount: Uint128,
    pub owner: String,
    pub unbonding_period: u64,
    pub xoro_decimals: u32,
    pub xoro_uri: Option<String>,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;

    match contract_version.contract.as_ref() {
        "oroswap-staking" => match contract_version.version.as_ref() {
            "1.1.0" | "2.0.0" | "2.1.0" | "1.2.0" | "2.2.0" => {
                // Try to load new config first (idempotent)
                if let Ok(_) = CONFIG.load(deps.storage) {
                    // Already migrated
                } else {
                    // Read old config and migrate it
                    let legacy_config_item: Item<ConfigV2_1_0> = Item::new("config");
                    let old_config = legacy_config_item.load(deps.storage)?;
                    let new_config = Config {
                        oro_denom: old_config.oro_denom,
                        xoro_denom: old_config.xoro_denom,
                        bootstrap_amount: old_config.bootstrap_amount,
                        owner: old_config.owner,
                        unbonding_period: old_config.unbonding_period,
                        xoro_decimals: old_config.xoro_decimals,
                        xoro_uri: old_config.xoro_uri,
                        paused: false, // Default to unpaused
                    };
                    CONFIG.save(deps.storage, &new_config)?;
                }
            }
            _ => {
                return Err(ContractError::MigrationError {});
            }
        },
        _ => {
            return Err(ContractError::MigrationError {});
        }
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}
