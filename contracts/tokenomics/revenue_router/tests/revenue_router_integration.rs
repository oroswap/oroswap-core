#![cfg(not(tarpaulin_include))]

use cosmwasm_std::{Addr, Uint128};

use oroswap_revenue_router::error::ContractError;

mod common;
use common::helper::{Helper, FIRST_HOP_RECEIVER, FORWARD_MEMO, USDC_DENOM};

// Note on Stargate coverage: upstream cw-multi-test (which the rest of the
// audit-scope contracts use, see fee_granter) doesn't implement Stargate.
// We assert that pre-Stargate logic behaves correctly (auth, balance, cap,
// pause, etc.) and that the Stargate hop is attempted on the happy path.
// The Stargate success + reply paths are covered by unit tests in
// src/testing.rs (test_reply_*) and exercised live on chain.

// ── Instantiate / config ──────────────────────────────────────────────────────

#[test]
fn test_instantiate_persists_destination_policy() {
    let owner = Addr::unchecked("owner");
    let helper = Helper::new(&owner).unwrap();

    let cfg = helper.query_config().unwrap();
    assert_eq!(cfg.owner, owner.to_string());
    assert_eq!(cfg.usdc_denom, USDC_DENOM);
    assert_eq!(cfg.first_hop_receiver, FIRST_HOP_RECEIVER);
    assert_eq!(cfg.forward_memo, FORWARD_MEMO);
    assert_eq!(cfg.max_bridge_per_tx, None);
    assert!(!cfg.paused);

    let preview = helper.query_preview().unwrap();
    assert_eq!(preview.receiver, FIRST_HOP_RECEIVER);
    assert_eq!(preview.memo, FORWARD_MEMO);
    assert_eq!(preview.channel, "channel-44");
}

// ── Keeper management ─────────────────────────────────────────────────────────

#[test]
fn test_keeper_lifecycle_and_pagination() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    helper.add_keeper(&owner, "zig1aaa00000000000000000000000000000000aaa").unwrap();
    helper.add_keeper(&owner, "zig1bbb00000000000000000000000000000000bbb").unwrap();
    helper.add_keeper(&owner, "zig1ccc00000000000000000000000000000000ccc").unwrap();

    let all = helper.query_keepers(None, None).unwrap();
    assert_eq!(all.keepers.len(), 3);

    let p1 = helper.query_keepers(None, Some(2)).unwrap();
    assert_eq!(p1.keepers.len(), 2);
    let cursor = p1.keepers.last().cloned().unwrap();

    let p2 = helper.query_keepers(Some(&cursor), Some(2)).unwrap();
    assert_eq!(p2.keepers.len(), 1);
    assert!(!p2.keepers.iter().any(|k| p1.keepers.contains(k)));

    helper.remove_keeper(&owner, "zig1bbb00000000000000000000000000000000bbb").unwrap();
    let after = helper.query_keepers(None, None).unwrap();
    assert_eq!(after.keepers.len(), 2);
}

#[test]
fn test_add_keeper_unauthorized() {
    let owner = Addr::unchecked("owner");
    let stranger = Addr::unchecked("zig1stranger00000000000000000000000000000st");
    let mut helper = Helper::new(&owner).unwrap();

    let err = helper.add_keeper(&stranger, "zig1someone").unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );
}

// ── Bridge negative paths (all enforced before the Stargate hop) ──────────────

#[test]
fn test_bridge_zero_balance() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let err = helper.bridge(&owner, None).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ZeroBalance {}
    );
}

#[test]
fn test_bridge_blocked_by_pause() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(100_000);

    helper.set_paused(&owner, true).unwrap();
    let err = helper.bridge(&owner, None).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Paused {}
    );
}

#[test]
fn test_bridge_unauthorized_caller() {
    let owner = Addr::unchecked("owner");
    let stranger = Addr::unchecked("zig1stranger00000000000000000000000000000st");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(100_000);

    let err = helper.bridge(&stranger, None).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );
}

#[test]
fn test_bridge_insufficient_balance() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(50_000);

    let err = helper.bridge(&owner, Some(100_000)).unwrap_err();
    assert!(matches!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::InsufficientBalance { .. }
    ));
}

#[test]
fn test_bridge_cap_rejects_overrun() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new_with_cap(&owner, Some(Uint128::new(50_000))).unwrap();
    helper.fund_contract(100_000);

    let err = helper.bridge(&owner, Some(75_000)).unwrap_err();
    assert!(matches!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::BridgeCapExceeded { .. }
    ));
}

// ── Bridge happy path + reply error branch (end-to-end) ──────────────────────

#[test]
fn test_bridge_reaches_stargate_and_reply_handles_failure() {
    // Upstream cw-multi-test doesn't route Stargate messages, so the
    // MsgTransfer the contract emits comes back as a submsg failure.
    // Because we use SubMsg::reply_always, the contract's reply handler
    // catches that failure and the overall tx still returns Ok — with
    // attributes describing what failed. This exercises the *full*
    // reply error path end-to-end (top-level bridge_usdc attrs +
    // bridge_reply attrs + ibc_transfer_status=failed + the upstream
    // error reason carried through).
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(100_000);

    let res = helper.bridge(&owner, None).unwrap();

    let attrs: Vec<(&str, &str)> = res
        .events
        .iter()
        .flat_map(|e| e.attributes.iter())
        .map(|a| (a.key.as_str(), a.value.as_str()))
        .collect();
    let has = |k: &str, v: &str| attrs.iter().any(|(ak, av)| *ak == k && *av == v);

    // Top-level bridge_usdc attributes, before submsg
    assert!(has("action", "bridge_usdc"));
    assert!(has("amount", "100000"));
    assert!(has("channel", "channel-44"));
    assert!(has("first_hop_receiver", FIRST_HOP_RECEIVER));

    // Reply attributes, after submsg failure
    assert!(has("action", "bridge_reply"));
    assert!(has("bridge_amount", "100000"));
    assert!(has("bridge_receiver", FIRST_HOP_RECEIVER));
    assert!(has("ibc_transfer_status", "failed"));

    // The upstream "Unexpected exec msg StargateMsg" error string is
    // surfaced as ibc_transfer_error — confirms reason carries through.
    let reason = attrs
        .iter()
        .find(|(k, _)| *k == "ibc_transfer_error")
        .map(|(_, v)| *v)
        .unwrap_or("");
    assert!(
        reason.to_lowercase().contains("stargate"),
        "expected ibc_transfer_error to reference Stargate, got: {reason}"
    );
}

// ── Ownership flow ────────────────────────────────────────────────────────────

#[test]
fn test_ownership_two_step_transfer() {
    let owner = Addr::unchecked("owner");
    let new_owner = Addr::unchecked("zig1newowner000000000000000000000000000new");
    let mut helper = Helper::new(&owner).unwrap();

    helper.propose_new_owner(&owner, &new_owner, 3_600).unwrap();
    helper.claim_ownership(&new_owner).unwrap();

    assert_eq!(helper.query_config().unwrap().owner, new_owner.to_string());
}

// ── EmergencyWithdraw ─────────────────────────────────────────────────────────

#[test]
fn test_emergency_withdraw_to_owner() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(200_000);

    let before = helper.query_balance(&owner, USDC_DENOM).unwrap();
    helper
        .emergency_withdraw(&owner, USDC_DENOM, None, None)
        .unwrap();
    let after = helper.query_balance(&owner, USDC_DENOM).unwrap();
    assert_eq!(after.u128() - before.u128(), 200_000);
}

#[test]
fn test_emergency_withdraw_pre_checks_balance() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(300_000);

    let err = helper
        .emergency_withdraw(&owner, USDC_DENOM, Some(900_000), None)
        .unwrap_err();
    match err.downcast::<ContractError>().unwrap() {
        ContractError::InsufficientBalance { requested, balance } => {
            assert_eq!(requested, "900000");
            assert_eq!(balance, "300000");
        }
        other => panic!("expected InsufficientBalance, got {other:?}"),
    }
}

#[test]
fn test_emergency_withdraw_unauthorized() {
    let owner = Addr::unchecked("owner");
    let stranger = Addr::unchecked("zig1stranger00000000000000000000000000000st");
    let mut helper = Helper::new(&owner).unwrap();
    helper.fund_contract(100_000);

    let err = helper
        .emergency_withdraw(&stranger, USDC_DENOM, None, None)
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );
}
