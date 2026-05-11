use cosmwasm_std::testing::{mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info};
use cosmwasm_std::{
    coins, from_json, Addr, Attribute, BankMsg, CosmosMsg, Reply, SubMsgResponse, SubMsgResult,
    Uint128,
};

use crate::contract::{execute, instantiate, migrate, query, reply, stargate_ibc_transfer, BRIDGE_REPLY_ID};
use crate::error::ContractError;
use crate::msg::{
    BalanceResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, KeepersResponse, MigrateMsg,
    PreviewBridgeResponse, QueryMsg,
};
use crate::state::{
    BridgeAttempt, CONFIG, DEFAULT_IBC_TIMEOUT_SECS, KEEPERS, LAST_BRIDGE_ATTEMPT,
};

const USDC_DENOM: &str = "ibc/5260516290F7883EC893AADA09A6B8CEC790F2EEF3196F440037908749785BE8";
const OWNER: &str = "zig1owner000000000000000000000000000000000";
const KEEPER: &str = "zig1keeper00000000000000000000000000000000";
const STRANGER: &str = "zig1stranger0000000000000000000000000000000";
const FIRST_HOP_RECEIVER: &str = "noble1receiver000000000000000000000000000000";
const FORWARD_MEMO: &str = r#"{"forward":{"channel":"channel-22","port":"transfer","receiver":"osmo1abc","retries":2,"timeout":"10m"}}"#;

fn default_instantiate_msg() -> InstantiateMsg {
    InstantiateMsg {
        owner: OWNER.to_string(),
        usdc_denom: USDC_DENOM.to_string(),
        ibc_channel: "channel-44".to_string(),
        ibc_timeout_secs: None,
        first_hop_receiver: FIRST_HOP_RECEIVER.to_string(),
        forward_memo: FORWARD_MEMO.to_string(),
        max_bridge_per_tx: None,
    }
}

fn empty_update_config() -> ExecuteMsg {
    ExecuteMsg::UpdateConfig {
        usdc_denom: None,
        ibc_channel: None,
        ibc_timeout_secs: None,
        first_hop_receiver: None,
        forward_memo: None,
        max_bridge_per_tx: None,
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
    assert_eq!(cfg.first_hop_receiver, FIRST_HOP_RECEIVER);
    assert_eq!(cfg.forward_memo, FORWARD_MEMO);
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
fn test_instantiate_rejects_empty_first_hop_receiver() {
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.first_hop_receiver = "   ".to_string();
    let err = instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidReceiver {});
}

#[test]
fn test_instantiate_rejects_malformed_memo() {
    let mut deps = mock_dependencies();
    for bad in &[
        "",
        "  ",
        "not-json",
        "{forward",          // unterminated brace
        r#"{"k":"v}"#,        // unterminated string
        r#"{"k":"v"}}"#,      // extra closing brace
        r#"[1,2,3]"#,         // array, not object
    ] {
        let mut msg = default_instantiate_msg();
        msg.forward_memo = bad.to_string();
        let err = instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidMemo {}, "expected InvalidMemo for: {bad:?}");
    }
}

#[test]
fn test_instantiate_emits_attributes() {
    let mut deps = mock_dependencies();
    let res = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        default_instantiate_msg(),
    )
    .unwrap();
    assert!(res.attributes.iter().any(|a| a.key == "action" && a.value == "instantiate"));
    assert!(res.attributes.iter().any(|a| a.key == "owner" && a.value == OWNER));
    assert!(res
        .attributes
        .iter()
        .any(|a| a.key == "first_hop_receiver" && a.value == FIRST_HOP_RECEIVER));
}

// ── Keeper management ─────────────────────────────────────────────────────────

#[test]
fn test_add_keeper_by_owner() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() },
    )
    .unwrap();

    let addr = Addr::unchecked(KEEPER);
    assert!(KEEPERS.has(&deps.storage, &addr));
}

#[test]
fn test_add_keeper_unauthorized() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(STRANGER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_add_keeper_no_duplicates() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    for _ in 0..3 {
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info(OWNER, &[]),
            ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() },
        )
        .unwrap();
    }
    let res: KeepersResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::Keepers { start_after: None, limit: None }).unwrap()).unwrap();
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
        deps.as_mut(),
        mock_env(),
        mock_info(STRANGER, &[]),
        ExecuteMsg::RemoveKeeper { keeper: KEEPER.to_string() },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

// ── BridgeUsdc ────────────────────────────────────────────────────────────────

#[test]
fn test_bridge_usdc_by_owner_full_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);

    let attrs: Vec<&Attribute> = res.attributes.iter().collect();
    assert!(attrs.iter().any(|a| a.key == "action" && a.value == "bridge_usdc"));
    assert!(attrs.iter().any(|a| a.key == "amount" && a.value == "5000000"));
    assert!(attrs.iter().any(|a| a.key == "channel" && a.value == "channel-44"));
    assert!(attrs.iter().any(|a| a.key == "keeper" && a.value == OWNER));
    assert!(attrs
        .iter()
        .any(|a| a.key == "first_hop_receiver" && a.value == FIRST_HOP_RECEIVER));
}

#[test]
fn test_bridge_usdc_by_keeper() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(KEEPER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap();
    assert_eq!(res.messages.len(), 1);
}

#[test]
fn test_bridge_usdc_partial_amount() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: Some(Uint128::new(2_000_000)) },
    )
    .unwrap();
    assert!(res.attributes.iter().any(|a| a.key == "amount" && a.value == "2000000"));
}

#[test]
fn test_bridge_usdc_unauthorized() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(STRANGER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_bridge_usdc_zero_balance() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::ZeroBalance {});
}

#[test]
fn test_bridge_usdc_insufficient_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: Some(Uint128::new(9_000_000)) },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::InsufficientBalance { .. }));
}

#[test]
fn test_bridge_usdc_explicit_zero_amount_rejected() {
    // Asking for amount=Some(0) is treated as a no-op error to avoid
    // emitting empty IBC transfers.
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: Some(Uint128::zero()) },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::ZeroBalance {});
}

#[test]
fn test_bridge_paused() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::SetPaused { paused: true }).unwrap();

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Paused {});
}

// ── UpdateConfig ──────────────────────────────────────────────────────────────

#[test]
fn test_update_config_by_owner() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: Some("channel-3".to_string()),
            ibc_timeout_secs: Some(300),
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: None,
        },
    )
    .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.ibc_channel, "channel-3");
    assert_eq!(cfg.ibc_timeout_secs, 300);
}

#[test]
fn test_update_config_destination_fields() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let new_receiver = "noble1newreceiver0000000000000000000000000".to_string();
    let new_memo = r#"{"noble":{"cctp":{"destination_chain_id":"1","mint_recipient":"0xdEAD"}}}"#.to_string();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: Some(new_receiver.clone()),
            forward_memo: Some(new_memo.clone()),
            max_bridge_per_tx: None,
        },
    )
    .unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.first_hop_receiver, new_receiver);
    assert_eq!(cfg.forward_memo, new_memo);
}

#[test]
fn test_update_config_rejects_bad_memo() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: None,
            forward_memo: Some("{not-json".to_string()),
            max_bridge_per_tx: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InvalidMemo {});
}

#[test]
fn test_update_config_rejects_empty_receiver() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: Some(String::new()),
            forward_memo: None,
            max_bridge_per_tx: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InvalidReceiver {});
}

#[test]
fn test_update_config_unauthorized() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(STRANGER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_two_step_ownership_transfer() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::ProposeNewOwner { owner: KEEPER.to_string(), expires_in: 1000 },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(KEEPER, &[]),
        ExecuteMsg::ClaimOwnership {},
    )
    .unwrap();

    assert_eq!(CONFIG.load(&deps.storage).unwrap().owner.as_str(), KEEPER);
}

// ── EmergencyWithdraw ─────────────────────────────────────────────────────────

#[test]
fn test_emergency_withdraw_full_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(3_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: None,
            recipient: None,
        },
    )
    .unwrap();

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
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: Some(Uint128::new(500_000)),
            recipient: Some(KEEPER.to_string()),
        },
    )
    .unwrap();

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
        deps.as_mut(),
        mock_env(),
        mock_info(STRANGER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: None,
            recipient: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_emergency_withdraw_zero_balance() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: None,
            recipient: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::ZeroBalance {});
}

// ── Queries ───────────────────────────────────────────────────────────────────

#[test]
fn test_query_config() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let res: ConfigResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();

    assert_eq!(res.owner, OWNER);
    assert_eq!(res.ibc_channel, "channel-44");
    assert_eq!(res.ibc_timeout_secs, DEFAULT_IBC_TIMEOUT_SECS);
    assert_eq!(res.first_hop_receiver, FIRST_HOP_RECEIVER);
    assert_eq!(res.forward_memo, FORWARD_MEMO);
    assert_eq!(res.paused, false);
}

#[test]
fn test_query_preview_bridge() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let res: PreviewBridgeResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::PreviewBridge {}).unwrap()).unwrap();

    assert_eq!(res.channel, "channel-44");
    assert_eq!(res.denom, USDC_DENOM);
    assert_eq!(res.receiver, FIRST_HOP_RECEIVER);
    assert_eq!(res.memo, FORWARD_MEMO);
    assert_eq!(res.timeout_secs, DEFAULT_IBC_TIMEOUT_SECS);
}

#[test]
fn test_query_usdc_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(7_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res: BalanceResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::UsdcBalance {}).unwrap()).unwrap();

    assert_eq!(res.amount.u128(), 7_000_000);
    assert_eq!(res.denom, USDC_DENOM);
}

#[test]
fn test_query_keepers() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]),
        ExecuteMsg::AddKeeper { keeper: KEEPER.to_string() }).unwrap();

    let res: KeepersResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::Keepers { start_after: None, limit: None }).unwrap()).unwrap();

    assert_eq!(res.keepers.len(), 1);
    assert_eq!(res.keepers[0], KEEPER);
}

// ── Protobuf encoding ─────────────────────────────────────────────────────────

#[test]
fn test_stargate_msg_type_url() {
    let msg = stargate_ibc_transfer(
        "zig1sender", "channel-44", "noble1receiver",
        1_000_000, USDC_DENOM, 9_999_999_999_000_000_000, FORWARD_MEMO,
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

#[test]
fn test_stargate_uses_config_receiver_and_memo() {
    // Sanity check that bridge_usdc actually wires Config values through —
    // not just *some* receiver / memo.
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap();
    if let CosmosMsg::Stargate { value, .. } = &res.messages[0].msg {
        let s = String::from_utf8_lossy(value.as_slice());
        assert!(s.contains(FIRST_HOP_RECEIVER), "stargate receiver should come from Config");
        assert!(s.contains(FORWARD_MEMO), "stargate memo should come from Config");
    } else {
        panic!("expected Stargate msg");
    }
}

// ── Additional coverage ────────────────────────────────────────────────────────

#[test]
fn test_update_config_stranded_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: Some("new_denom".to_string()),
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: None,
        },
    )
    .unwrap_err();
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

    let res: KeepersResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::Keepers { start_after: None, limit: None }).unwrap()).unwrap();
    assert_eq!(res.keepers.len(), 2);
    assert!(res.keepers.contains(&KEEPER.to_string()));
    assert!(res.keepers.contains(&"zig1keeper2".to_string()));
}

#[test]
fn test_ibc_timeout_overflow() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: Some(u64::MAX),
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: None,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InvalidTimeout {});
}

// ── Per-tx bridge cap ─────────────────────────────────────────────────────────

#[test]
fn test_bridge_cap_default_uncapped() {
    let mut deps = mock_dependencies_with_balance(&coins(10_000_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());
    // No cap at instantiate → arbitrarily large transfer succeeds
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap();
}

#[test]
fn test_instantiate_with_cap() {
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(1_000_000));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();

    let cfg = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(cfg.max_bridge_per_tx, Some(Uint128::new(1_000_000)));
}

#[test]
fn test_instantiate_rejects_zero_cap() {
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::zero());
    let err = instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidBridgeCap {});
}

#[test]
fn test_bridge_respects_cap() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(2_000_000));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();

    // Within cap → ok
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: Some(Uint128::new(2_000_000)) },
    )
    .unwrap();
}

#[test]
fn test_bridge_explicit_amount_above_cap_rejected() {
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(2_000_000));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: Some(Uint128::new(2_500_000)) },
    )
    .unwrap_err();
    match err {
        ContractError::BridgeCapExceeded { requested, cap } => {
            assert_eq!(requested, "2500000");
            assert_eq!(cap, "2000000");
        }
        other => panic!("expected BridgeCapExceeded, got {other:?}"),
    }
}

#[test]
fn test_bridge_full_balance_above_cap_rejected() {
    // amount: None → bridge_amount = balance. If balance > cap, reject so
    // the keeper has to explicitly request a capped amount.
    let mut deps = mock_dependencies_with_balance(&coins(5_000_000, USDC_DENOM));
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(1_000_000));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: None },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::BridgeCapExceeded { .. }));
}

#[test]
fn test_update_config_can_set_cap() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: Some(Some(Uint128::new(500_000))),
        },
    )
    .unwrap();
    assert_eq!(
        CONFIG.load(&deps.storage).unwrap().max_bridge_per_tx,
        Some(Uint128::new(500_000))
    );
}

#[test]
fn test_update_config_can_clear_cap() {
    // After setting, the owner can clear back to uncapped using Some(None).
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(100));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();
    assert_eq!(
        CONFIG.load(&deps.storage).unwrap().max_bridge_per_tx,
        Some(Uint128::new(100))
    );

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: Some(None),
        },
    )
    .unwrap();
    assert_eq!(CONFIG.load(&deps.storage).unwrap().max_bridge_per_tx, None);
}

#[test]
fn test_update_config_rejects_zero_cap() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::UpdateConfig {
            usdc_denom: None,
            ibc_channel: None,
            ibc_timeout_secs: None,
            first_hop_receiver: None,
            forward_memo: None,
            max_bridge_per_tx: Some(Some(Uint128::zero())),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InvalidBridgeCap {});
}

#[test]
fn test_update_config_leaves_cap_unchanged_when_none() {
    // Outer None means "field not supplied" — the cap should NOT be touched.
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(999));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();

    execute(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), empty_update_config()).unwrap();
    assert_eq!(
        CONFIG.load(&deps.storage).unwrap().max_bridge_per_tx,
        Some(Uint128::new(999))
    );
}

#[test]
fn test_preview_bridge_exposes_cap() {
    let mut deps = mock_dependencies();
    let mut msg = default_instantiate_msg();
    msg.max_bridge_per_tx = Some(Uint128::new(123));
    instantiate(deps.as_mut(), mock_env(), mock_info(OWNER, &[]), msg).unwrap();

    let res: PreviewBridgeResponse =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::PreviewBridge {}).unwrap()).unwrap();
    assert_eq!(res.max_bridge_per_tx, Some(Uint128::new(123)));
}

#[test]
fn test_migrate() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());

    let res = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    assert_eq!(res.messages.len(), 0);

    cw2::set_contract_version(deps.as_mut().storage, "wrong_name", "1.0.0").unwrap();
    let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
    assert_eq!(err, ContractError::MigrationError {});
}

// ── Reply handler ─────────────────────────────────────────────────────────────

fn seed_bridge_attempt(storage: &mut dyn cosmwasm_std::Storage) {
    LAST_BRIDGE_ATTEMPT
        .save(
            storage,
            &BridgeAttempt {
                amount: Uint128::new(123_456),
                receiver: "noble1someaddr".to_string(),
                channel: "channel-44".to_string(),
                denom: USDC_DENOM.to_string(),
            },
        )
        .unwrap();
}

fn ok_reply() -> Reply {
    Reply {
        id: BRIDGE_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse { events: vec![], data: None }),
    }
}

fn err_reply(reason: &str) -> Reply {
    Reply {
        id: BRIDGE_REPLY_ID,
        result: SubMsgResult::Err(reason.to_string()),
    }
}

#[test]
fn test_reply_success_emits_context_and_status() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    seed_bridge_attempt(&mut deps.storage);

    let res = reply(deps.as_mut(), mock_env(), ok_reply()).unwrap();

    let attr_for = |k: &str| {
        res.attributes
            .iter()
            .find(|a| a.key == k)
            .map(|a| a.value.clone())
    };
    assert_eq!(attr_for("action"), Some("bridge_reply".to_string()));
    assert_eq!(attr_for("bridge_amount"), Some("123456".to_string()));
    assert_eq!(attr_for("bridge_receiver"), Some("noble1someaddr".to_string()));
    assert_eq!(attr_for("bridge_channel"), Some("channel-44".to_string()));
    assert_eq!(attr_for("bridge_denom"), Some(USDC_DENOM.to_string()));
    assert_eq!(attr_for("ibc_transfer_status"), Some("queued".to_string()));
}

#[test]
fn test_reply_failure_emits_error_reason() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    seed_bridge_attempt(&mut deps.storage);

    let res = reply(deps.as_mut(), mock_env(), err_reply("channel closed")).unwrap();

    let attr_for = |k: &str| {
        res.attributes
            .iter()
            .find(|a| a.key == k)
            .map(|a| a.value.clone())
    };
    assert_eq!(attr_for("ibc_transfer_status"), Some("failed".to_string()));
    assert_eq!(attr_for("ibc_transfer_error"), Some("channel closed".to_string()));
    assert_eq!(attr_for("bridge_amount"), Some("123456".to_string()));
}

#[test]
fn test_reply_clears_bridge_attempt_storage() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    seed_bridge_attempt(&mut deps.storage);
    assert!(LAST_BRIDGE_ATTEMPT.may_load(&deps.storage).unwrap().is_some());

    reply(deps.as_mut(), mock_env(), ok_reply()).unwrap();
    assert!(LAST_BRIDGE_ATTEMPT.may_load(&deps.storage).unwrap().is_none());

    // Same on the error branch.
    seed_bridge_attempt(&mut deps.storage);
    reply(deps.as_mut(), mock_env(), err_reply("boom")).unwrap();
    assert!(LAST_BRIDGE_ATTEMPT.may_load(&deps.storage).unwrap().is_none());
}

#[test]
fn test_reply_no_attempt_recorded_does_not_panic() {
    // Defensive: even if LAST_BRIDGE_ATTEMPT is somehow missing, reply
    // should still succeed and emit the status attribute.
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    let res = reply(deps.as_mut(), mock_env(), ok_reply()).unwrap();
    assert!(res
        .attributes
        .iter()
        .any(|a| a.key == "ibc_transfer_status" && a.value == "queued"));
}

#[test]
fn test_reply_unknown_id_is_noop() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    let res = reply(
        deps.as_mut(),
        mock_env(),
        Reply {
            id: 99,
            result: SubMsgResult::Ok(SubMsgResponse { events: vec![], data: None }),
        },
    )
    .unwrap();
    assert!(res.attributes.is_empty());
}

#[test]
fn test_bridge_usdc_writes_attempt_before_submsg() {
    // Verify the storage slot is populated for the reply handler to read.
    let mut deps = mock_dependencies_with_balance(&coins(1_000_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::BridgeUsdc { amount: Some(Uint128::new(750_000)) },
    )
    .unwrap();

    let attempt = LAST_BRIDGE_ATTEMPT.load(&deps.storage).unwrap();
    assert_eq!(attempt.amount, Uint128::new(750_000));
    assert_eq!(attempt.receiver, FIRST_HOP_RECEIVER);
    assert_eq!(attempt.channel, "channel-44");
    assert_eq!(attempt.denom, USDC_DENOM);
}

// ── Keepers pagination ────────────────────────────────────────────────────────

fn add_keepers(deps: &mut cosmwasm_std::OwnedDeps<cosmwasm_std::MemoryStorage, cosmwasm_std::testing::MockApi, cosmwasm_std::testing::MockQuerier>, n: usize) {
    for i in 0..n {
        let k = format!("zig1keeper{:040}", i);
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info(OWNER, &[]),
            ExecuteMsg::AddKeeper { keeper: k },
        )
        .unwrap();
    }
}

#[test]
fn test_keepers_pagination_default_limit() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    add_keepers(&mut deps, 35);

    let res: KeepersResponse = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Keepers { start_after: None, limit: None },
        )
        .unwrap(),
    )
    .unwrap();
    // DEFAULT_KEEPER_LIMIT = 30
    assert_eq!(res.keepers.len(), 30);
}

#[test]
fn test_keepers_pagination_explicit_limit() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    add_keepers(&mut deps, 10);

    let res: KeepersResponse = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Keepers { start_after: None, limit: Some(5) },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(res.keepers.len(), 5);
}

#[test]
fn test_keepers_pagination_caps_max() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    add_keepers(&mut deps, 150);

    let res: KeepersResponse = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Keepers { start_after: None, limit: Some(10_000) },
        )
        .unwrap(),
    )
    .unwrap();
    // MAX_KEEPER_LIMIT = 100
    assert_eq!(res.keepers.len(), 100);
}

#[test]
fn test_keepers_pagination_start_after_walks() {
    let mut deps = mock_dependencies();
    do_instantiate(deps.as_mut());
    add_keepers(&mut deps, 5);

    let page1: KeepersResponse = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Keepers { start_after: None, limit: Some(2) },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(page1.keepers.len(), 2);

    let cursor = page1.keepers.last().cloned().unwrap();
    let page2: KeepersResponse = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Keepers { start_after: Some(cursor.clone()), limit: Some(2) },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(page2.keepers.len(), 2);
    // pages must not overlap
    assert!(page2.keepers.iter().all(|k| !page1.keepers.contains(k)));
}

// ── EmergencyWithdraw insufficient-balance pre-check ─────────────────────────

#[test]
fn test_emergency_withdraw_insufficient_balance() {
    let mut deps = mock_dependencies_with_balance(&coins(500_000, USDC_DENOM));
    do_instantiate(deps.as_mut());

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(OWNER, &[]),
        ExecuteMsg::EmergencyWithdraw {
            denom: USDC_DENOM.to_string(),
            amount: Some(Uint128::new(1_000_000)),
            recipient: None,
        },
    )
    .unwrap_err();
    match err {
        ContractError::InsufficientBalance { requested, balance } => {
            assert_eq!(requested, "1000000");
            assert_eq!(balance, "500000");
        }
        other => panic!("expected InsufficientBalance, got {other:?}"),
    }
}
