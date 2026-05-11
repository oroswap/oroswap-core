use crate::contract::{execute, CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::migrate::migrate;
use crate::msg::{ExecuteMsg, MigrateMsg};
use crate::state::Config;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coin, Addr};

/// Construct a `Config` with the given owner and a synthetic mint denom,
/// skipping the create-denom reply path (which requires a proto encoded
/// response that the cosmwasm-std mock can't produce).
fn seed_config(deps: &mut cosmwasm_std::OwnedDeps<impl cosmwasm_std::Storage, impl cosmwasm_std::Api, impl cosmwasm_std::Querier>, owner: &str, lock_denom: &str, mint_denom: &str) {
    crate::state::CONFIG
        .save(
            &mut deps.storage,
            &Config {
                lock_denom: lock_denom.to_string(),
                mint_denom: mint_denom.to_string(),
                owner: Addr::unchecked(owner),
                paused: false,
            },
        )
        .unwrap();
}

#[test]
fn test_lock_unlock() {
    let mut deps = mock_dependencies();
    let lock_denom = "uzig";
    let mint_denom = "coin.contract.wzig";
    seed_config(&mut deps, "owner", lock_denom, mint_denom);

    // Lock emits the mint sub-message
    let info = mock_info("user1", &[coin(1_000, lock_denom)]);
    let res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Lock { receiver: None }).unwrap();
    assert_eq!(res.messages.len(), 1);

    // Unlock emits burn + bank send
    let info = mock_info("user1", &[coin(500, mint_denom)]);
    let res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Unlock { receiver: None }).unwrap();
    assert_eq!(res.messages.len(), 2);
}

#[test]
fn test_pause_unpause() {
    let mut deps = mock_dependencies();
    let lock_denom = "uzig";
    let mint_denom = "coin.contract.wzig";
    seed_config(&mut deps, "owner", lock_denom, mint_denom);

    // Owner pauses
    let owner_info = mock_info("owner", &[]);
    execute(deps.as_mut(), mock_env(), owner_info.clone(), ExecuteMsg::Pause {}).unwrap();

    // Lock fails while paused
    let user_info = mock_info("user1", &[coin(1_000, lock_denom)]);
    let err = execute(deps.as_mut(), mock_env(), user_info, ExecuteMsg::Lock { receiver: None })
        .unwrap_err();
    assert!(err.to_string().contains("Contract is paused"));

    // Owner unpauses and lock succeeds
    execute(deps.as_mut(), mock_env(), owner_info, ExecuteMsg::Unpause {}).unwrap();
    let user_info = mock_info("user1", &[coin(1_000, lock_denom)]);
    execute(deps.as_mut(), mock_env(), user_info, ExecuteMsg::Lock { receiver: None }).unwrap();
}

#[test]
fn test_migrate_same_name() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, CONTRACT_VERSION).unwrap();

    let res = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let new_name = res
        .attributes
        .iter()
        .find(|a| a.key == "new_contract_name")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(new_name, CONTRACT_NAME);
}

#[test]
fn test_migrate_wrong_name_rejected() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "some-other-contract", "0.0.1").unwrap();

    let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
    assert_eq!(err, ContractError::MigrationError {});
}
