use cosmwasm_std::testing::{mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info};
use cosmwasm_std::{coins, from_json, Attribute, BankMsg, CosmosMsg, Uint128};

use crate::contract::{execute, instantiate, query, stargate_ibc_transfer};
use crate::error::ContractError;
use crate::msg::{
    BalanceResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, KeepersResponse, QueryMsg,
};
use crate::state::{CONFIG, DEFAULT_IBC_TIMEOUT_SECS};

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
        dest_chain_id: "osmo-test-5".to_string(),
        dest_usdc_denom: "ibc/DE6792CF9E521F6AD6E9A4BDF6225C9571A3B74ACC0A529F92BC5122A39D2E58"
            .to_string(),
        dest_receiver: "osmo1receiver000000000000000000000000000000".to_string(),
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
    assert!(cfg.keepers.is_empty());
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

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert!(cfg.keepers.iter().any(|k| k.as_str() == KEEPER));
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
    assert_eq!(CONFIG.load(&deps.storage).unwrap().keepers.len(), 1);
}

#[test]
fn test_remove_keeper() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();
    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::RemoveKeeper { keeper: KEEPER.to_string() }).unwrap();

    assert!(CONFIG.load(&deps.storage).unwrap().keepers.is_empty());
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
    assert!(matches!(res.messages[0].msg, CosmosMsg::Stargate { .. }));

    let attrs: Vec<&Attribute> = res.attributes.iter().collect();
    assert!(attrs.iter().any(|a| a.key == "action" && a.value == "bridge_usdc"));
    assert!(attrs.iter().any(|a| a.key == "amount" && a.value == "5000000"));
    assert!(attrs.iter().any(|a| a.key == "channel" && a.value == "channel-44"));
    assert!(attrs.iter().any(|a| a.key == "receiver" && a.value == NOBLE_RECEIVER));
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
            owner: None,
            usdc_denom: None,
            dest_chain_id: Some("1".to_string()),
            dest_usdc_denom: None,
            dest_receiver: None,
            ibc_channel: Some("channel-3".to_string()),
            ibc_timeout_secs: Some(300),
        },
    ).unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.dest_chain_id, "1");
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
            owner: None, usdc_denom: None, dest_chain_id: None,
            dest_usdc_denom: None, dest_receiver: None,
            ibc_channel: None, ibc_timeout_secs: None,
        },
    ).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_update_config_transfer_ownership() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            owner: Some(KEEPER.to_string()),
            usdc_denom: None, dest_chain_id: None, dest_usdc_denom: None,
            dest_receiver: None, ibc_channel: None, ibc_timeout_secs: None,
        },
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
