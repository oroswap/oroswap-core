use cosmwasm_std::testing::{mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info};
use cosmwasm_std::{coins, from_json, Addr, Attribute, BankMsg, CosmosMsg, Uint128};

use crate::contract::{execute, instantiate, query, stargate_ibc_transfer, migrate};
use crate::error::ContractError;
use crate::msg::{
    BalanceResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, KeepersResponse, QueryMsg, MigrateMsg
};
use crate::state::{CONFIG, DEFAULT_IBC_TIMEOUT_SECS, KEEPERS};

const USDC_DENOM: &str = "ibc/5260516290F7883EC893AADA09A6B8CEC790F2EEF3196F440037908749785BE8";
const OWNER: &str = "zig1owner000000000000000000000000000000000";
const KEEPER: &str = "zig1keeper00000000000000000000000000000000";
const STRANGER: &str = "zig1stranger0000000000000000000000000000000";
const NOBLE_RECEIVER: &str = "noble1receiver000000000000000000000000000000";
const MEMO: &str = r#"{"forward":{"channel":"channel-22","port":"transfer","receiver":"osmo1abc","retries":2,"timeout":9999999999}}"#;

fn default_instantiate_msg() -> InstantiateMsg {
    InstantiateMsg {
        owner: OWNER.to_string(),
        usdc_denom: USDC_DENOM.to_string(),
        ibc_channel: "channel-44".to_string(),
        ibc_timeout_secs: None,
    }
}

fn do_instantiate(deps: cosmwasm_std::DepsMut) {
    instantiate(deps, mock_env(), mock_info(OWNER, &[]), default_instantiate_msg()).unwrap();
}

// ── Instantiate ───────────────────────────────────────────────────────────────

#[test]
fn test_instantiate_stores_config() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.owner, OWNER);
    assert_eq!(cfg.usdc_denom, USDC_DENOM);
    assert_eq!(cfg.ibc_channel, "channel-44");
    assert_eq!(cfg.ibc_timeout_secs, DEFAULT_IBC_TIMEOUT_SECS);
    assert!(KEEPERS.is_empty(&deps.storage));
}

#[test]
fn test_instantiate_custom_timeout() {
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.ibc_timeout_secs = Some(1200);
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();
    assert_eq!(CONFIG.load(&deps.storage).unwrap().ibc_timeout_secs, 1200);
}

#[test]
fn test_instantiate_emits_attributes() {
    let mut deps = mock_dependencies();
    let res = instantiate(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]), default_instantiate_msg(),
    ).unwrap();
    assert!(res.attributes.iter().any(|a| a.key == "action" && a.value == "instantiate"));
    assert!(res.attributes.iter().any(|a| a.key == "owner" && a.value == OWNER));
}

// ── Keeper management ─────────────────────────────────────────────────────────

#[test]
fn test_add_keeper_by_owner() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() },
    ).unwrap();

    let addr = Addr::unchecked(KEEPER);
    assert!(KEEPERS.has(&deps.storage, &addr));
}

#[test]
fn test_add_keeper_unauthorized() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(STRANGER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() },
    ).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_add_keeper_no_duplicates() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    for _ in 0..3 {
        execute(
            deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
            ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() },
        ).unwrap();
    }
    let res: KeepersResponse = from_json(query(deps.as_ref(), mock_env(), QueryMsg::Keepers {}).unwrap()).unwrap();
    assert_eq!(res.keepers.len(), 1);
}

#[test]
fn test_remove_keeper() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();
    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::RemoveKeeper { keeper: KEEPER.to_string() }).unwrap();

    assert!(KEEPERS.is_empty(&deps.storage));
}

#[test]
fn test_remove_keeper_unauthorized() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(STRANGER, &[]),
        ExecuteMsg::RemoveKeeper { keeper: KEEPER.to_string() },
    ).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

// ── BridgeUsdc ────────────────────────────────────────────────────────────────

#[test]
fn test_bridge_usdc_by_owner_full_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: None,
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap();

    assert_eq!(res.messages.len(), 1);

    let attrs: Vec<&Attribute> = res.attributes.iter().collect();
    assert!(attrs.iter().any(|a| a.key == "action" && a.value == "bridge_usdc"));
    assert!(attrs.iter().any(|a| a.key == "amount" && a.value == "5000000"));
    assert!(attrs.iter().any(|a| a.key == "channel" && a.value == "channel-44"));
    assert!(attrs.iter().any(|a| a.key == "keeper" && a.value == OWNER));
}

#[test]
fn test_bridge_usdc_by_keeper() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();

    let res = execute(
        deps.as_mut(), mock_env(), mock_info(KEEPER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: None,
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap();
    assert_eq!(res.messages.len(), 1);
}

#[test]
fn test_bridge_usdc_partial_amount() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: Some(Uint128::new(2_000_000)),
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap();
    assert!(res.attributes.iter().any(|a| a.key == "amount" && a.value == "2000000"));
}

#[test]
fn test_bridge_usdc_unauthorized() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(STRANGER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: None,
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_bridge_usdc_zero_balance() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: None,
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::ZeroBalance {});
}

#[test]
fn test_bridge_usdc_insufficient_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: Some(Uint128::new(9_000_000)),
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap_err();
    assert!(matches!(err, ContractError::InsufficientBalance { .. }));
}

// ── UpdateConfig ──────────────────────────────────────────────────────────────

#[test]
fn test_update_config_by_owner() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: Some("channel-3".to_string()),
            ibc_timeout_secs: Some(300),
        },
    ).unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.ibc_channel, "channel-3");
    assert_eq!(cfg.ibc_timeout_secs, 300);
}

#[test]
fn test_update_config_unauthorized() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(STRANGER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None, ibc_timeout_secs: None,
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_two_step_ownership_transfer() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    // 1. Propose
    execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::ProposeNewOwner { owner: KEEPER.to_string(), expires_in: 1000 },
    ).unwrap();

    // 2. Claim
    execute(
        deps.as_mut(), mock_env(), mock_info(KEEPER, &[]),
        ExecuteMsg::ClaimOwnership {},
    ).unwrap();

    assert_eq!(CONFIG.load(&deps.storage).unwrap().owner.as_str(), KEEPER);
}

// ── EmergencyWithdraw ─────────────────────────────────────────────────────────

#[test]
fn test_emergency_withdraw_full_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(3_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: None,
            recipient: None,
        },
    ).unwrap();

    assert_eq!(res.messages.len(), 1);
    if let CosmosMsg::Bank(BankMsg::Send { to_address, amount }) = &res.messages[0].msg {
        assert_eq!(to_address, OWNER);
        assert_eq!(amount[0].amount.u128(), 3_000_000);
    } else {
        panic!("expected BankMsg::Send");
    }
}

#[test]
fn test_emergency_withdraw_to_recipient() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: Some(Uint128::new(500_000)),
            recipient: Some(KEEPER.to_string()),
        },
    ).unwrap();

    if let CosmosMsg::Bank(BankMsg::Send { to_address, amount }) = &res.messages[0].msg {
        assert_eq!(to_address, KEEPER);
        assert_eq!(amount[0].amount.u128(), 500_000);
    } else {
        panic!("expected BankMsg::Send");
    }
}

#[test]
fn test_emergency_withdraw_unauthorized() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(STRANGER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: None,
            recipient: None,
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_emergency_withdraw_zero_balance() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: None,
            recipient: None,
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::ZeroBalance {});
}

// ── Queries ───────────────────────────────────────────────────────────────────

#[test]
fn test_query_config() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let res: ConfigResponse = from_json(
        query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()
    ).unwrap();

    assert_eq!(res.owner, OWNER);
    assert_eq!(res.ibc_channel, "channel-44");
    assert_eq!(res.ibc_timeout_secs, DEFAULT_IBC_TIMEOUT_SECS);
    assert_eq!(res.paused, false);
}

#[test]
fn test_query_usdc_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(7_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res: BalanceResponse = from_json(
        query(deps.as_ref(), mock_env(), QueryMsg::UsdcBalance {}).unwrap()
    ).unwrap();

    assert_eq!(res.amount.u128(), 7_000_000);
    assert_eq!(res.denom, USDC_DENOM);
}

#[test]
fn test_query_keepers() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();

    let res: KeepersResponse = from_json(
        query(deps.as_ref(), mock_env(), QueryMsg::Keepers {}).unwrap()
    ).unwrap();

    assert_eq!(res.keepers.len(), 1);
    assert_eq!(res.keepers[0], KEEPER);
}

// ── Protobuf encoding ─────────────────────────────────────────────────────────

#[test]
fn test_stargate_msg_type_url() {
    let msg = stargate_ibc_transfer(
        "zig1sender", "channel-44", "noble1receiver",
        1_000_000, USDC_DENOM, 9_999_999_999_000_000_000, MEMO,
    );
    if let CosmosMsg::Stargate { type_url, value } = msg {
        assert_eq!(type_url, "/ibc.applications.transfer.v1.MsgTransfer");
        assert!(!value.is_empty());
    } else {
        panic!("expected Stargate msg");
    }
}

#[test]
fn test_stargate_msg_no_memo() {
    let msg = stargate_ibc_transfer(
        "zig1sender", "channel-44", "noble1receiver",
        1_000_000, USDC_DENOM, 9_999_999_999_000_000_000, "",
    );
    assert!(matches!(msg, CosmosMsg::Stargate { .. }));
}

// ── Additional coverage ────────────────────────────────────────────────────────

#[test]
fn test_update_config_stranded_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());
    
    let err = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: Some("new_denom".to_string()),
            ibc_channel: None, ibc_timeout_secs: None,
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::StrandedBalance {});
}

#[test]
fn test_multiple_keepers() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    
    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();
    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: "zig1keeper2".to_string() }).unwrap();
        
    let res: KeepersResponse = from_json(query(deps.as_ref(), mock_env(), QueryMsg::Keepers {}).unwrap()).unwrap();
    assert_eq!(res.keepers.len(), 2);
    assert!(res.keepers.contains(&KEEPER.to_string()));
    assert!(res.keepers.contains(&"zig1keeper2".to_string()));
}

#[test]
fn test_bridge_usdc_zero_amount() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());
    
    let res = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc {
            amount: Some(Uint128::zero()),
            memo: MEMO.to_string(),
            receiver: NOBLE_RECEIVER.to_string(),
        },
    ).unwrap();
    assert!(res.attributes.iter().any(|a| a.key == "amount" && a.value == "0"));
}

#[test]
fn test_ibc_timeout_overflow() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    
    let err = execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None, ibc_channel: None,
            ibc_timeout_secs: Some(u64::MAX),
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::InvalidTimeout {});
}

#[test]
fn test_migrate() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    // Should succeed
    let res = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    assert_eq!(res.messages.len(), 0);
    
    // Corrupt name and try again
    cw2::set_contract_version(deps.as_mut().storage, "wrong_name", "1.0.0").unwrap();
    let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
    assert_eq!(err, ContractError::MigrationError {});
}

#[test]
fn test_stargate_protobuf_detailed() {
    let msg = stargate_ibc_transfer(
        "zig1sender", "channel-44", "noble1receiver",
        1_000_000, USDC_DENOM, 9_999_999_999_000_000_000, MEMO,
    );
    if let CosmosMsg::Stargate { value, .. } = msg {
        let s = String::from_utf8_lossy(value.as_slice());
        assert!(s.contains("channel-44"));
        assert!(s.contains("transfer")); // source port
        assert!(s.contains("zig1sender"));
        assert!(s.contains(USDC_DENOM));
        assert!(s.contains("noble1receiver"));
        assert!(s.contains(MEMO));
    } else {
        panic!("expected Stargate msg");
    }
}
