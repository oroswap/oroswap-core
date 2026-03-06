#![cfg(not(tarpaulin_include))]

use cosmwasm_std::{coins, coin, Addr};
use cw_multi_test::Executor;
use cw_utils::PaymentError;

use oroswap_wrapper::error::ContractError;
use oroswap_wrapper::msg::ExecuteMsg;

use crate::common::helper::{Helper, LOCK_DENOM, MINT_SUBDENOM};
mod common;

// ---------------------------------------------------------------------------
// test_instantiate
// ---------------------------------------------------------------------------

/// Verify that after instantiation the config is stored correctly and the
/// tokenfactory token denom is created with the expected naming convention.
#[test]
fn test_instantiate() {
    let owner = Addr::unchecked("owner");
    let helper = Helper::new(&owner).unwrap();

    let config = helper.query_config().unwrap();

    assert_eq!(config.lock_denom, LOCK_DENOM);
    // ZigChain denom pattern: "coin.<contract_address>.<subdenom>"
    assert_eq!(
        config.mint_denom,
        format!("coin.{}.{}", helper.wrapper, MINT_SUBDENOM)
    );
    assert_eq!(config.owner, owner.to_string());
    assert!(!config.paused);
}

// ---------------------------------------------------------------------------
// test_lock_and_unlock
// ---------------------------------------------------------------------------

/// Full happy-path: Alice locks IBC tokens → gets mint tokens 1:1.
/// Then she unlocks half → burns mint tokens and gets IBC tokens back.
#[test]
fn test_lock_and_unlock() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let mint_denom = helper.mint_denom.clone();
    let wrapper_addr = helper.wrapper.clone();

    let alice = Addr::unchecked("alice");
    helper.give_lock_tokens(10_000, &alice);

    // --- Lock 5000 ---
    helper.lock(&alice, 5_000).unwrap();

    // Alice should now have 5000 mint tokens and 5000 lock tokens remaining
    let alice_mint = helper.query_balance(&alice, &mint_denom).unwrap();
    let alice_lock = helper.query_balance(&alice, LOCK_DENOM).unwrap();
    assert_eq!(alice_mint.u128(), 5_000, "alice mint after lock");
    assert_eq!(alice_lock.u128(), 5_000, "alice lock remaining");

    // Contract should hold 5000 lock tokens
    let contract_lock = helper
        .query_balance(&wrapper_addr, LOCK_DENOM)
        .unwrap();
    assert_eq!(contract_lock.u128(), 5_000, "contract lock balance");

    // TotalLocked query should agree
    let total_locked = helper.query_total_locked().unwrap();
    assert_eq!(total_locked.u128(), 5_000, "TotalLocked query");

    // --- Unlock 2000 ---
    helper.unlock(&alice, 2_000).unwrap();

    // Alice burns 2000 mint tokens and receives 2000 lock tokens back
    let alice_mint = helper.query_balance(&alice, &mint_denom).unwrap();
    let alice_lock = helper.query_balance(&alice, LOCK_DENOM).unwrap();
    assert_eq!(alice_mint.u128(), 3_000, "alice mint after partial unlock");
    assert_eq!(alice_lock.u128(), 7_000, "alice lock after partial unlock");

    // Contract holds 3000 lock tokens now
    let contract_lock = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    assert_eq!(contract_lock.u128(), 3_000, "contract after unlock");

    // TotalLocked == mint supply (1:1 invariant)
    let total_locked = helper.query_total_locked().unwrap();
    assert_eq!(total_locked.u128(), 3_000, "invariant after unlock");
}

// ---------------------------------------------------------------------------
// test_wrong_denom_lock
// ---------------------------------------------------------------------------

/// Sending the wrong denom to Lock must fail.
#[test]
fn test_wrong_denom_lock() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let bad_denom = "uzig";
    helper.mint_coin(&owner, coin(10_000, bad_denom));

    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::Lock { receiver: None },
            &coins(1_000, bad_denom),
        )
        .unwrap_err();

    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(LOCK_DENOM.to_string()))
    );
}

// ---------------------------------------------------------------------------
// test_wrong_denom_unlock
// ---------------------------------------------------------------------------

/// Sending the wrong denom to Unlock must fail.
#[test]
fn test_wrong_denom_unlock() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // First lock some tokens so alice has ibc tokens to play with
    let alice = Addr::unchecked("alice");
    helper.give_lock_tokens(1_000, &alice);

    // Try to unlock with the raw lock denom instead of the mint denom
    let err = helper
        .app
        .execute_contract(
            alice.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::Unlock { receiver: None },
            &coins(100, LOCK_DENOM),
        )
        .unwrap_err();

    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(
            helper.mint_denom.clone()
        ))
    );
}

// ---------------------------------------------------------------------------
// test_custom_receiver
// ---------------------------------------------------------------------------

/// When `receiver` is set, minted tokens go to that address, not the sender.
#[test]
fn test_custom_receiver() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let mint_denom = helper.mint_denom.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");
    helper.give_lock_tokens(5_000, &alice);

    // Alice locks 1000 and sends mint tokens to Bob
    helper.lock_to(&alice, 1_000, &bob).unwrap();

    let bob_mint = helper.query_balance(&bob, &mint_denom).unwrap();
    let alice_mint = helper.query_balance(&alice, &mint_denom).unwrap();
    assert_eq!(bob_mint.u128(), 1_000, "bob received mint tokens");
    assert_eq!(alice_mint.u128(), 0, "alice has no mint tokens");

    // Bob unlocks to Alice
    helper.unlock_to(&bob, 500, &alice).unwrap();
    let alice_lock = helper.query_balance(&alice, LOCK_DENOM).unwrap();
    // Alice had 4000 lock tokens remaining; now 4000 + 500 = 4500
    assert_eq!(alice_lock.u128(), 4_500, "alice received lock tokens back");
}

// ---------------------------------------------------------------------------
// test_pause_unpause
// ---------------------------------------------------------------------------

/// Only the owner can pause/unpause. Lock/unlock are blocked while paused.
#[test]
fn test_pause_unpause() {
    let owner = Addr::unchecked("owner");
    let alice = Addr::unchecked("alice");
    let mut helper = Helper::new(&owner).unwrap();
    helper.give_lock_tokens(5_000, &alice);

    // Non-owner cannot pause
    let err = helper.pause(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    // Owner pauses
    helper.pause(&owner).unwrap();
    let config = helper.query_config().unwrap();
    assert!(config.paused);

    // Lock is blocked
    let err = helper.lock(&alice, 1_000).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Unlock with mint tokens (first need some — do it after unpause)
    // Non-owner cannot unpause
    let err = helper.unpause(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    // Owner unpauses
    helper.unpause(&owner).unwrap();
    let config = helper.query_config().unwrap();
    assert!(!config.paused);

    // Lock works again
    helper.lock(&alice, 1_000).unwrap();

    // Pause again; unlock is also blocked
    helper.pause(&owner).unwrap();
    let err = helper.unlock(&alice, 100).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Restore
    helper.unpause(&owner).unwrap();
    helper.unlock(&alice, 100).unwrap();
}

// ---------------------------------------------------------------------------
// test_1_to_1_invariant
// ---------------------------------------------------------------------------

/// At every point the contract's `lock_denom` balance must equal the circulating
/// supply of `mint_denom`. We exercise this with multiple users.
#[test]
fn test_1_to_1_invariant() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let mint_denom = helper.mint_denom.clone();
    let wrapper_addr = helper.wrapper.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");
    helper.give_lock_tokens(10_000, &alice);
    helper.give_lock_tokens(10_000, &bob);

    // Both lock
    helper.lock(&alice, 3_000).unwrap();
    helper.lock(&bob, 7_000).unwrap();

    let locked = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    let minted = helper
        .app
        .wrap()
        .query_supply(&mint_denom)
        .unwrap()
        .amount;
    assert_eq!(locked, minted, "invariant after two locks");

    // Alice partially unlocks
    helper.unlock(&alice, 1_000).unwrap();

    let locked = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    let minted = helper
        .app
        .wrap()
        .query_supply(&mint_denom)
        .unwrap()
        .amount;
    assert_eq!(locked, minted, "invariant after alice unlock");

    // Bob fully unlocks
    helper.unlock(&bob, 7_000).unwrap();

    let locked = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    let minted = helper
        .app
        .wrap()
        .query_supply(&mint_denom)
        .unwrap()
        .amount;
    assert_eq!(locked, minted, "invariant after bob full unlock");
    assert_eq!(locked.u128(), 2_000, "2000 alice tokens remain locked");
}

// ---------------------------------------------------------------------------
// test_update_config
// ---------------------------------------------------------------------------

/// Owner can transfer ownership.
#[test]
fn test_update_config() {
    let owner = Addr::unchecked("owner");
    let new_owner = Addr::unchecked("new_owner");
    let alice = Addr::unchecked("alice");
    let mut helper = Helper::new(&owner).unwrap();

    // Non-owner cannot update config
    let err = helper
        .app
        .execute_contract(
            alice.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::UpdateConfig {
                owner: Some(alice.to_string()),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    // Owner transfers to new_owner
    helper
        .app
        .execute_contract(
            owner.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::UpdateConfig {
                owner: Some(new_owner.to_string()),
            },
            &[],
        )
        .unwrap();

    let config = helper.query_config().unwrap();
    assert_eq!(config.owner, new_owner.to_string());

    // Old owner can no longer pause
    let err = helper.pause(&owner).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    // New owner can pause
    helper.pause(&new_owner).unwrap();
    let config = helper.query_config().unwrap();
    assert!(config.paused);
}
