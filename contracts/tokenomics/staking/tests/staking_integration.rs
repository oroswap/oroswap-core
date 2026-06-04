#![cfg(not(tarpaulin_include))]

//! Integration tests for the 1:1 ORO stake-registry staking contract.
//!
//! Post-refactor invariants this suite is the primary safety net for:
//!   * Per-user stake lives in `STAKES`; total supply in `TOTAL_STAKES`.
//!     There is **no tokenfactory denom** owned by the staking contract,
//!     so any test that previously asserted on a bank balance of the share
//!     token has been deleted or rewritten to query `StakeOf` / `TotalShares`.
//!   * `Leave` takes a typed `amount` + `recipient` and is funded by the
//!     contract's own state (no `must_pay`).
//!   * `STAKES[contract_address]` is always zero — the contract holds no
//!     bootstrap principal. The first-depositor inflation attack is
//!     structurally impossible because there are no shares to inflate:
//!     every Enter mints exactly its ORO amount.
//!   * `EnterWithHook` no longer exists.

use cosmwasm_std::{coin, coins, from_json, Addr, Empty, Uint128};
use cw_multi_test::{Contract, ContractWrapper, Executor};
use cw_utils::PaymentError;

use oroswap::staking::{Config, ExecuteMsg, QueryMsg, StakingResponse};
use oroswap_staking::error::ContractError;

use crate::common::helper::{Helper, ORO_DENOM, UNBONDING_PERIOD};

mod common;

// ─── Instantiate ─────────────────────────────────────────────────────────────

#[test]
fn test_instantiate() {
    let owner = Addr::unchecked("owner");

    let helper = Helper::new(&owner).unwrap();

    let response: Config = helper
        .app
        .wrap()
        .query_wasm_smart(&helper.staking, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(response.owner, Addr::unchecked("owner"));
    assert_eq!(response.oro_denom, ORO_DENOM.to_string());
    assert_eq!(response.unbonding_period, UNBONDING_PERIOD);
    assert!(!response.paused);

    // No bootstrap stake: contract address holds zero, total supply is zero.
    let contract_stake = helper.query_stake_of(&helper.staking).unwrap();
    assert_eq!(contract_stake, Uint128::zero());
    let total = helper.query_total_shares().unwrap();
    assert_eq!(total, Uint128::zero());
}

// ─── Enter: payment validation + stake accounting ────────────────────────────

#[test]
fn test_invalid_denom() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();

    let bad_denom = "bad/denom";
    helper.mint_coin(&owner, coin(1_000, bad_denom));

    // Try to stake bad denom only.
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Enter {},
            &coins(1_000u128, bad_denom),
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(ORO_DENOM.to_string()))
    );

    // Try to stake bad denom along with ORO — must reject multi-denom payment.
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Enter {},
            &[coin(1_000u128, bad_denom), coin(1_000u128, ORO_DENOM)],
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MultipleDenoms {})
    );

    // NOTE: There is no "wrong denom on Leave" path anymore — Leave is a typed
    // amount, not a token transfer.
}

#[test]
fn test_enter_and_leave() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let staking = helper.staking.clone();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    // Stake Alice's 1100 ORO → STAKES[alice] = 1100 (1:1).
    let resp_data = helper.stake(&alice, 1_100).unwrap().data.unwrap();
    let staking_resp: StakingResponse = from_json(&resp_data).unwrap();
    assert_eq!(
        staking_resp,
        StakingResponse {
            oro_amount: 1_100u128.into(),
            share_amount: 1_100u128.into(),
        }
    );

    let alice_stake = helper.query_stake_of(&alice).unwrap();
    assert_eq!(alice_stake.u128(), 1_100);

    // Contract's ORO balance: just Alice's 1_100 (no bootstrap).
    let contract_oro = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(contract_oro.u128(), 1_100);

    // Total stake supply == Alice's deposit.
    let total_shares = helper.query_total_shares().unwrap();
    assert_eq!(total_shares.u128(), 1_100);

    // Leave 10 — typed amount, no must_pay.
    let resp_data = helper.unstake(&alice, 10).unwrap().data.unwrap();
    let staking_resp: StakingResponse = from_json(&resp_data).unwrap();
    assert_eq!(
        staking_resp,
        StakingResponse {
            oro_amount: 10u128.into(),
            share_amount: 10u128.into(),
        }
    );

    // STAKES[alice] decremented by 10.
    let alice_stake = helper.query_stake_of(&alice).unwrap();
    assert_eq!(alice_stake.u128(), 1_090);

    // Alice's ORO balance unchanged until Claim — Leave only queues unbonding.
    let alice_oro = helper.query_balance(&alice, ORO_DENOM).unwrap();
    assert_eq!(alice_oro.u128(), 8_900);

    // Fast forward past the unbonding period and Claim.
    helper.next_block(UNBONDING_PERIOD);
    helper.claim(&alice).unwrap();

    let alice_oro = helper.query_balance(&alice, ORO_DENOM).unwrap();
    assert_eq!(alice_oro.u128(), 8_910);

    // Contract's ORO balance: 1_100 - 10 paid out.
    let contract_oro = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(contract_oro.u128(), 1_090);
}

/// A single user calling Enter twice accumulates 1:1 in STAKES.
#[test]
fn test_stake_increment_on_enter() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    helper.stake(&alice, 100).unwrap();
    let after_first = helper.query_stake_of(&alice).unwrap();
    assert_eq!(after_first.u128(), 100);

    helper.stake(&alice, 50).unwrap();
    let after_second = helper.query_stake_of(&alice).unwrap();
    assert_eq!(after_second.u128(), 150);

    let total_shares = helper.query_total_shares().unwrap();
    assert_eq!(total_shares.u128(), 150);
}

/// Partial Leave leaves the remainder intact and flushes pending rewards
/// for the leaving slice.
#[test]
fn test_partial_leave_keeps_remainder() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    helper.stake(&alice, 100).unwrap();
    helper.unstake(&alice, 1).unwrap();

    let alice_stake = helper.query_stake_of(&alice).unwrap();
    assert_eq!(alice_stake.u128(), 99);

    let total_shares = helper.query_total_shares().unwrap();
    assert_eq!(total_shares.u128(), 99);

    // Pending rewards must be empty (no reward stream → nothing to flush).
    let pending = helper.query_pending_rewards(&alice).unwrap();
    assert!(pending.is_empty());
}

/// Negative test for `ContractError::InsufficientStake`.
#[test]
fn test_leave_with_insufficient_stake_rejected() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    helper.stake(&alice, 50).unwrap();

    let err = helper.unstake(&alice, 100).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(
            root,
            ContractError::InsufficientStake {
                available,
                requested,
            } if available == Uint128::new(50) && requested == Uint128::new(100)
        ),
        "expected InsufficientStake {{ available: 50, requested: 100 }}, got {root:?}",
    );

    // State is unchanged on the failed Leave.
    let alice_stake = helper.query_stake_of(&alice).unwrap();
    assert_eq!(alice_stake.u128(), 50);
}

// ─── 1:1 invariant under "donation" ──────────────────────────────────────────

/// Donated ORO must not affect anyone's stake. Under the 1:1 model a bank-
/// send of ORO directly to the contract sits in the contract's balance as
/// uncredited excess — it does NOT inflate existing stakers' principal and
/// it does NOT change the share-mint ratio for new stakers.
#[test]
fn test_no_exchange_rate_growth_on_donation() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let staking = helper.staking.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");
    helper.give_oro(100, &alice);
    helper.give_oro(100, &bob);

    // Alice enters 100.
    helper.stake(&alice, 100).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 100);

    // Somebody bank-sends 1_000_000 ORO directly to the contract. This is a
    // donation — the contract neither credits it to a user nor uses it to
    // change the share-mint ratio.
    helper.give_oro(1_000_000, &owner);
    helper
        .bank_send(&owner, &staking, coins(1_000_000, ORO_DENOM))
        .unwrap();

    // Bob enters 100. He gets exactly 100 stake — donation is irrelevant.
    helper.stake(&bob, 100).unwrap();
    assert_eq!(helper.query_stake_of(&bob).unwrap().u128(), 100);

    // Total stake remains alice+bob only.
    assert_eq!(helper.query_total_shares().unwrap().u128(), 200);

    // Both unstake all and claim → each gets exactly 100 ORO back.
    let alice_before = helper.query_balance(&alice, ORO_DENOM).unwrap().u128();
    let bob_before = helper.query_balance(&bob, ORO_DENOM).unwrap().u128();

    helper.unstake(&alice, 100).unwrap();
    helper.unstake(&bob, 100).unwrap();
    helper.next_block(UNBONDING_PERIOD);
    helper.claim(&alice).unwrap();
    helper.claim(&bob).unwrap();

    let alice_after = helper.query_balance(&alice, ORO_DENOM).unwrap().u128();
    let bob_after = helper.query_balance(&bob, ORO_DENOM).unwrap().u128();
    assert_eq!(alice_after - alice_before, 100, "alice must get exactly her principal back");
    assert_eq!(bob_after - bob_before, 100, "bob must get exactly his principal back");

    // The donated 1M ORO is still sitting in the contract balance: total
    // contract balance = 1_000_000 (donation) + 0 (live stake) + 0 (locked).
    let contract_balance = helper.query_balance(&staking, ORO_DENOM).unwrap().u128();
    assert_eq!(
        contract_balance, 1_000_000,
        "donated ORO must remain untouched in contract balance",
    );

    // OroNonRecoverable: admin cannot sweep it either — the guard is the
    // intentional safety mechanism even though there are no live stakers
    // right now.
    let err = helper
        .withdraw_excess_tokens(&owner, ORO_DENOM, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::OroNonRecoverable {}),
        "expected OroNonRecoverable, got {root:?}",
    );
}

// ─── FIND-006 regression ─────────────────────────────────────────────────────

/// FIND-006 regression: stakes live only in the `STAKES: Map<&Addr, Uint128>`
/// storage. There is no transferable share denom, so a non-staker cannot
/// `Leave` against another user's stake.
#[test]
fn test_find006_attack_uncallable() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");

    helper.give_oro(10_000, &alice);

    // Alice stakes 5000.
    helper.stake(&alice, 5_000).unwrap();
    let alice_stake = helper.query_stake_of(&alice).unwrap();
    assert_eq!(alice_stake.u128(), 5_000);

    // Bob has nothing.
    let bob_stake = helper.query_stake_of(&bob).unwrap();
    assert_eq!(bob_stake.u128(), 0);

    // No share-denom exists to grab. Even a synthesized plausible denom string
    // carries zero balance for every actor.
    let pre_refactor_share = format!("coin.{}.share", &helper.staking);
    assert_eq!(
        helper.query_balance(&alice, &pre_refactor_share).unwrap().u128(),
        0,
    );
    assert_eq!(
        helper
            .query_balance(&helper.staking.clone(), &pre_refactor_share)
            .unwrap()
            .u128(),
        0,
    );

    // Bank transfers of any denom do not mutate STAKES.
    let bob_stake_before = helper.query_stake_of(&bob).unwrap();
    helper.give_oro(1, &bob);
    let bob_stake_after_oro_recv = helper.query_stake_of(&bob).unwrap();
    assert_eq!(bob_stake_before, bob_stake_after_oro_recv);

    // Bob with no stake → Leave fails with InsufficientStake (FIND-006 closed).
    let err = helper.unstake(&bob, 5_000).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::InsufficientStake { .. }),
        "Bob with no stake must NOT be able to Leave on Alice's stake; got {root:?}",
    );

    // Alice's stake unchanged after Bob's failed attack.
    let alice_stake_after = helper.query_stake_of(&alice).unwrap();
    assert_eq!(
        alice_stake_after, alice_stake,
        "Alice's STAKES must be untouched by Bob's bogus Leave",
    );

    // ───── Step 4: contract address holds zero stake (no bootstrap).
    let contract_stake = helper.query_stake_of(&helper.staking.clone()).unwrap();
    assert_eq!(
        contract_stake,
        Uint128::zero(),
        "no bootstrap stake under the 1:1 model — contract address holds nothing",
    );
}

// ─── Reward accrual under shifting stake ─────────────────────────────────────

/// Spec invariant: rewards accrue against the user's stake **at the time of
/// accrual**, not at the time of claim. This test stakes 100, lets a stream
/// accrue, leaves 50, then asserts:
///   1. The reward earned over the first window is computed against the full
///      100 stake.
///   2. After the partial Leave, further accrual is against the remaining 50.
///   3. Total payout (sum of unbonding-entry reward_coins + future pending)
///      equals the sum of pro-rata accrual under each weight, no over- or
///      under-pay.
#[test]
fn test_reward_accrual_unchanged_after_stake_change() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let alice = Addr::unchecked("alice");

    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 100).unwrap();

    // Open a 1000-USDC stream over 1000 seconds.
    let usdc = "uusdc";
    helper.mint_coin(&owner, coin(1_000, usdc));
    helper.add_reward(&owner, coin(1_000, usdc), 1_000).unwrap();

    // Half the stream elapses with stake = 100 (alice only).
    helper.next_block(500);

    // Snapshot pending: alice's share is 100/100 of 500 USDC = 500.
    let pending_before_leave = helper.query_pending_rewards(&alice).unwrap();
    let alice_usdc_before = pending_before_leave
        .iter()
        .find(|c| c.denom == usdc)
        .map(|c| c.amount.u128())
        .unwrap_or(0);
    assert!(
        (499..=501).contains(&alice_usdc_before),
        "expected ~500 USDC pending at half-stream, got {alice_usdc_before}",
    );

    // Alice Leaves 50 — must flush the accrued USDC into the unbonding entry
    // for those 50 stake and continue accruing on the remaining 50.
    helper.unstake(&alice, 50).unwrap();
    let alice_pending_after_leave = helper.query_pending_rewards(&alice).unwrap();
    assert!(
        alice_pending_after_leave.iter().all(|c| c.amount.is_zero()),
        "pending must be flushed into the unbonding entry on Leave",
    );

    // Second window: stake = 50 (alice only). Half-stream remaining = 500 USDC.
    // Alice's share = 50/50 * 500 = 500.
    helper.next_block(500);
    let alice_pending_window2 = helper.query_pending_rewards(&alice).unwrap();
    let alice_usdc_window2 = alice_pending_window2
        .iter()
        .find(|c| c.denom == usdc)
        .map(|c| c.amount.u128())
        .unwrap_or(0);
    assert!(
        (499..=501).contains(&alice_usdc_window2),
        "expected ~500 USDC pending in second window, got {alice_usdc_window2}",
    );

    // Now finalise: unbonding period elapses → claim the first slice;
    // alice also unstakes the remainder → claim everything.
    helper.next_block(UNBONDING_PERIOD);
    let usdc_before_claim = helper.query_balance(&alice, usdc).unwrap().u128();
    helper.claim(&alice).unwrap();
    let usdc_after_first_claim = helper.query_balance(&alice, usdc).unwrap().u128();
    let first_window_payout = usdc_after_first_claim - usdc_before_claim;
    assert_eq!(
        first_window_payout, alice_usdc_before,
        "first-window claim must pay exactly the snapshot value",
    );

    // Alice unstakes the rest and waits.
    helper.unstake(&alice, 50).unwrap();
    helper.next_block(UNBONDING_PERIOD);
    let usdc_before_second_claim = helper.query_balance(&alice, usdc).unwrap().u128();
    helper.claim(&alice).unwrap();
    let usdc_after_second_claim = helper.query_balance(&alice, usdc).unwrap().u128();
    let second_window_payout = usdc_after_second_claim - usdc_before_second_claim;

    // Total Alice received must be the sum of both window snapshots — no
    // double-credit, no lost credit across the stake change.
    let total_alice = first_window_payout + second_window_payout;
    assert!(
        total_alice >= alice_usdc_before + alice_usdc_window2 - 2
            && total_alice <= alice_usdc_before + alice_usdc_window2 + 2,
        "total payout {total_alice} must equal sum of window snapshots {} (±1 floor)",
        alice_usdc_before + alice_usdc_window2,
    );
}

/// With N stakers and a funded reward stream of total T, the sum of payouts
/// across all stakers must equal T (modulo per-step floor rounding).
#[test]
fn test_no_overpay_with_multiple_stakers() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");

    helper.give_oro(10_000, &alice);
    helper.give_oro(10_000, &bob);
    helper.stake(&alice, 50).unwrap();
    helper.stake(&bob, 50).unwrap();

    let bar = "ubar";
    helper.mint_coin(&owner, coin(100, bar));
    // 100 BAR over 100 seconds → rps = 1, predictable accrual.
    helper.add_reward(&owner, coin(100, bar), 100).unwrap();

    // Whole stream elapses.
    helper.next_block(100);

    // Both leave their full stakes.
    helper.unstake(&alice, 50).unwrap();
    helper.unstake(&bob, 50).unwrap();

    helper.next_block(UNBONDING_PERIOD);

    let alice_bar_before = helper.query_balance(&alice, bar).unwrap().u128();
    let bob_bar_before = helper.query_balance(&bob, bar).unwrap().u128();
    helper.claim(&alice).unwrap();
    helper.claim(&bob).unwrap();
    let alice_bar_after = helper.query_balance(&alice, bar).unwrap().u128();
    let bob_bar_after = helper.query_balance(&bob, bar).unwrap().u128();

    let alice_received = alice_bar_after - alice_bar_before;
    let bob_received = bob_bar_after - bob_bar_before;
    let total_paid = alice_received + bob_received;

    // Stakes: alice=50, bob=50, total_shares=100. Each owns 1/2 of 100 BAR = 50.
    // Total paid out should be 100 (the whole stream).
    assert!(
        total_paid <= 100,
        "total payout {total_paid} must not exceed funded stream of 100",
    );
    assert!(
        alice_received > 0 && bob_received > 0,
        "both stakers must receive non-zero BAR (alice={alice_received}, bob={bob_received})",
    );
    assert_eq!(
        alice_received, bob_received,
        "equal stake → equal payout (alice={alice_received}, bob={bob_received})",
    );
}

// ─── Two-staker integration: existing-behavior smoke ─────────────────────────

#[test]
fn should_work_with_more_than_one_participant() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let staking = helper.staking.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");

    helper.give_oro(10_000, &alice);
    helper.give_oro(10_000, &bob);

    helper.stake(&alice, 2_000).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 2_000);

    helper.stake(&bob, 10).unwrap();
    assert_eq!(helper.query_stake_of(&bob).unwrap().u128(), 10);

    // Contract holds alice + bob in ORO. No bootstrap.
    let contract_oro = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(contract_oro.u128(), 2_000 + 10);

    // External donation of 20 ORO directly to the contract — under the 1:1
    // model this is uncredited excess and does NOT shift the mint ratio.
    helper.give_oro(20, &staking);

    // Alice stakes 10 more ORO. She gets exactly 10 stake (1:1).
    let before_third_stake = helper.query_stake_of(&alice).unwrap().u128();
    helper.stake(&alice, 10).unwrap();
    let after_third_stake = helper.query_stake_of(&alice).unwrap().u128();
    let minted = after_third_stake - before_third_stake;
    assert_eq!(
        minted, 10,
        "1:1 stake mint must equal the ORO sent (donations are uncredited); got minted={minted}",
    );

    // Bob unstakes 5. Shouldn't fail.
    helper.unstake(&bob, 5).unwrap();
    assert_eq!(helper.query_stake_of(&bob).unwrap().u128(), 5);

    helper.next_block(UNBONDING_PERIOD);
    helper.claim(&bob).unwrap();

    // Bob's ORO went up by exactly the unbonded ORO (5).
    let bob_oro = helper.query_balance(&bob, ORO_DENOM).unwrap();
    // bob started with 10_000, staked 10 → balance 9_990, unbonded 5 → +5 → 9_995.
    assert_eq!(bob_oro.u128(), 9_995, "bob received exactly the unbonded ORO");
}

// ─── Pause / unpause ─────────────────────────────────────────────────────────

#[test]
fn test_pause_unpause() {
    let owner = Addr::unchecked("owner");
    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");

    let mut helper = Helper::new(&owner).unwrap();
    helper.give_oro(10_000, &alice);
    helper.give_oro(10_000, &bob);

    // 1. Only owner (or authority) can pause.
    let err = helper.pause(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    helper.pause(&owner).unwrap();

    // 2. Actions blocked when paused.
    let err = helper.stake(&alice, 1_000).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Setup for leave/claim — stake during unpaused window.
    helper.unpause(&owner).unwrap();
    helper.stake(&alice, 1_000).unwrap();
    helper.pause(&owner).unwrap();

    let err = helper.unstake(&alice, 100).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Setup for claim error.
    helper.unpause(&owner).unwrap();
    helper.unstake(&alice, 100).unwrap();
    helper.pause(&owner).unwrap();

    let err = helper.claim(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // 3. Unpause restores actions.
    helper.unpause(&owner).unwrap();
    helper.stake(&bob, 1_000).unwrap();
    helper.unstake(&bob, 100).unwrap();

    helper.next_block(UNBONDING_PERIOD);
    helper.claim(&alice).unwrap();
    helper.claim(&bob).unwrap();
}

// ─── Same-version migrate is a no-op ─────────────────────────────────────────

/// Same-version migrate is a no-op against the in-flight test instance. The
/// genuine legacy-bytes path cannot be exercised through cw-multi-test
/// because contract storage is namespaced behind the wasm keeper. That path
/// is covered by the unit test in `migrate.rs` which writes legacy bytes
/// directly into `mock_dependencies()` storage. (Audit N-2.)
#[test]
fn test_migration_same_version() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let staking_addr = helper.staking.clone();

    use oroswap::staking::MigrateMsg;
    fn staking_contract() -> Box<dyn Contract<Empty>> {
        Box::new(
            ContractWrapper::new_with_empty(
                oroswap_staking::contract::execute,
                oroswap_staking::contract::instantiate,
                oroswap_staking::contract::query,
            )
            .with_reply_empty(oroswap_staking::contract::reply)
            .with_migrate_empty(oroswap_staking::migrate::migrate),
        )
    }
    let new_code_id = helper.app.store_code(staking_contract());

    helper
        .app
        .migrate_contract(owner.clone(), staking_addr.clone(), &MigrateMsg {}, new_code_id)
        .unwrap();

    let config: Config = helper
        .app
        .wrap()
        .query_wasm_smart(&staking_addr, &QueryMsg::Config {})
        .unwrap();
    assert!(!config.paused);
}

// ─── Reward accrual sanity (multi-denom, multi-staker) ───────────────────────

#[test]
fn test_incentives() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");
    let usdc_denom = "coin.osmo123.usdc";

    helper.give_oro(100_000, &alice);
    helper.give_oro(100_000, &bob);
    helper.mint_coin(&owner, coin(10_000, usdc_denom));

    // 1. Alice stakes 10_000 ORO → STAKES[alice] = 10_000 (1:1).
    helper.stake(&alice, 10_000).unwrap();

    // 2. Admin opens two reward streams for 100 seconds each.
    helper.add_reward(&owner, coin(1_000, ORO_DENOM), 100).unwrap();
    helper.add_reward(&owner, coin(10_000, usdc_denom), 100).unwrap();

    // 3. Half the stream elapses. Alice owns 100% of stake (10_000/10_000).
    helper.next_block(50);

    let pending = helper.query_pending_rewards(&alice).unwrap();
    let oro_pending = pending
        .iter()
        .find(|c| c.denom == ORO_DENOM)
        .unwrap()
        .amount
        .u128();
    let usdc_pending = pending
        .iter()
        .find(|c| c.denom == usdc_denom)
        .unwrap()
        .amount
        .u128();
    // 1000 * 50/100 * 10_000/10_000 = 500; 10_000 * 50/100 * 10_000/10_000 = 5000.
    assert!((499..=501).contains(&oro_pending), "oro_pending={oro_pending}");
    assert!((4999..=5001).contains(&usdc_pending), "usdc_pending={usdc_pending}");

    // 4. Bob stakes — Alice's pending is stashed at this point.
    helper.stake(&bob, 10_000).unwrap();

    // 5. Second half of the stream elapses.
    helper.next_block(50);

    let alice_pending = helper.query_pending_rewards(&alice).unwrap();
    let bob_pending = helper.query_pending_rewards(&bob).unwrap();
    let alice_oro = alice_pending
        .iter()
        .find(|c| c.denom == ORO_DENOM)
        .unwrap()
        .amount
        .u128();
    let bob_oro = bob_pending
        .iter()
        .find(|c| c.denom == ORO_DENOM)
        .unwrap()
        .amount
        .u128();
    // Alice ≈ 500 + (1000 * 50/100 * 10000/20000) = 500+250 = 750.
    // Bob ≈ 1000 * 50/100 * 10000/20000 = 250.
    assert!((748..=752).contains(&alice_oro), "alice_oro={alice_oro}");
    assert!((248..=252).contains(&bob_oro), "bob_oro={bob_oro}");

    // 6. Alice Leaves all → pending must move into the unbonding entry.
    helper.unstake(&alice, 10_000).unwrap();
    let alice_after = helper.query_pending_rewards(&alice).unwrap();
    assert!(
        alice_after.iter().all(|c| c.amount.is_zero()),
        "pending must be flushed on Leave",
    );

    // 7. Wait + Claim → Alice receives principal + accrued rewards.
    helper.next_block(UNBONDING_PERIOD);
    let alice_oro_before = helper.query_balance(&alice, ORO_DENOM).unwrap().u128();
    helper.claim(&alice).unwrap();
    let alice_oro_after = helper.query_balance(&alice, ORO_DENOM).unwrap().u128();
    let alice_usdc_after = helper.query_balance(&alice, usdc_denom).unwrap().u128();

    // Principal 10_000 + ~750 ORO reward.
    let delta = alice_oro_after - alice_oro_before;
    assert!((10_748..=10_752).contains(&delta), "alice ORO delta={delta}");
    // USDC ≈ 5000 + 2500 = ~7500.
    assert!(
        (7498..=7502).contains(&alice_usdc_after),
        "alice_usdc_after={alice_usdc_after}",
    );
}

// ─── PAUSE_AUTHORITIES (audit N-3 follow-up) ────────────────────────────────

#[test]
fn test_pause_authority_can_pause_but_not_unpause() {
    let owner = Addr::unchecked("owner");
    let authority = Addr::unchecked("authority");
    let mut helper = Helper::new(&owner).unwrap();

    helper
        .add_pause_authorities(&owner, vec![authority.to_string()])
        .unwrap();
    let list = helper.query_pause_authorities().unwrap();
    assert_eq!(list, vec![authority.to_string()]);

    helper.pause(&authority).unwrap();

    let err = helper.unpause(&authority).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::NoUnpauseAuthority {}),
        "expected NoUnpauseAuthority, got {root:?}",
    );

    helper.unpause(&owner).unwrap();
}

#[test]
fn test_pause_authority_management_owner_only() {
    let owner = Addr::unchecked("owner");
    let mallory = Addr::unchecked("mallory");
    let authority = Addr::unchecked("authority");
    let mut helper = Helper::new(&owner).unwrap();

    // Non-owner cannot add authorities.
    let err = helper
        .add_pause_authorities(&mallory, vec![authority.to_string()])
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert_eq!(root, ContractError::Unauthorized {});

    // Bogus address rejected with typed error.
    let err = helper
        .add_pause_authorities(&owner, vec!["".to_string()])
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::InvalidPauseAuthority { .. }),
        "expected InvalidPauseAuthority, got {root:?}",
    );

    // Add, then remove.
    helper
        .add_pause_authorities(&owner, vec![authority.to_string()])
        .unwrap();
    assert_eq!(helper.query_pause_authorities().unwrap().len(), 1);

    helper
        .remove_pause_authorities(&owner, vec![authority.to_string()])
        .unwrap();
    assert!(helper.query_pause_authorities().unwrap().is_empty());

    // Removed authority can no longer pause.
    let err = helper.pause(&authority).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert_eq!(root, ContractError::Unauthorized {});
}

// ─── WithdrawExcessTokens ────────────────────────────────────────────────────
//
// ORO (principal) and registered reward denoms cannot be swept.

#[test]
fn test_withdraw_excess_oro_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    // Bank-send 5_000 ORO directly to the contract.
    helper.give_oro(5_000, &owner);
    helper
        .bank_send(&owner, &helper.staking.clone(), coins(5_000, ORO_DENOM))
        .unwrap();

    // Owner cannot recover ORO.
    let err = helper
        .withdraw_excess_tokens(&owner, ORO_DENOM, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::OroNonRecoverable {}),
        "expected OroNonRecoverable, got {root:?}",
    );

    // Non-owner is rejected before the denom check.
    let mallory = Addr::unchecked("mallory");
    let err = helper
        .withdraw_excess_tokens(&mallory, ORO_DENOM, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert_eq!(root, ContractError::Unauthorized {});
}

#[test]
fn test_withdraw_excess_non_oro_full_balance() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let bogus = "stray/usdc";
    helper.mint_coin(&owner, coin(7_500, bogus));
    helper
        .bank_send(&owner, &helper.staking.clone(), coins(7_500, bogus))
        .unwrap();

    let owner_before = helper.query_balance(&owner, bogus).unwrap().u128();
    let contract_before = helper
        .query_balance(&helper.staking.clone(), bogus)
        .unwrap()
        .u128();
    assert_eq!(contract_before, 7_500);

    helper.withdraw_excess_tokens(&owner, bogus, None).unwrap();

    let owner_after = helper.query_balance(&owner, bogus).unwrap().u128();
    let contract_after = helper
        .query_balance(&helper.staking.clone(), bogus)
        .unwrap()
        .u128();
    assert_eq!(owner_after - owner_before, 7_500);
    assert_eq!(contract_after, 0);

    // Second call — nothing to withdraw.
    let err = helper
        .withdraw_excess_tokens(&owner, bogus, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::NoExcessToWithdraw {}),
        "expected NoExcessToWithdraw, got {root:?}",
    );
}

#[test]
fn test_withdraw_excess_reward_denom_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let usdc = "usdc";
    helper.mint_coin(&owner, coin(5_000, usdc));
    helper.add_reward(&owner, coin(5_000, usdc), 1_000).unwrap();

    let err = helper
        .withdraw_excess_tokens(&owner, usdc, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::DenomInRewardStream { ref denom } if denom == usdc),
        "expected DenomInRewardStream {{ denom: \"usdc\" }}, got {root:?}",
    );

    // Contract still holds the USDC.
    let contract_usdc = helper
        .query_balance(&helper.staking.clone(), usdc)
        .unwrap()
        .u128();
    assert_eq!(contract_usdc, 5_000);
}

#[test]
fn test_withdraw_excess_non_oro_custom_recipient() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let bogus = "stray/usdt";
    helper.mint_coin(&owner, coin(2_000, bogus));
    helper
        .bank_send(&owner, &helper.staking.clone(), coins(2_000, bogus))
        .unwrap();

    let beneficiary = Addr::unchecked("beneficiary");
    helper
        .withdraw_excess_tokens(&owner, bogus, Some(beneficiary.to_string()))
        .unwrap();

    assert_eq!(
        helper.query_balance(&beneficiary, bogus).unwrap().u128(),
        2_000,
    );
    assert_eq!(helper.query_balance(&owner, bogus).unwrap().u128(), 0);
}

// ─── Too-many-unbonding-entries typed variant ───────────────────────────────

/// Audit N-3 regression: the 11th `Leave` from the same address must fail
/// with the typed `TooManyUnbondingEntries { max: 10 }` variant.
#[test]
fn test_too_many_unbonding_entries_typed_error() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(11 * 1_000, &alice);
    helper.stake(&alice, 11 * 1_000).unwrap();

    let stake = helper.query_stake_of(&alice).unwrap().u128();
    let per_leave = stake / 11;

    for _ in 0..10 {
        helper.unstake(&alice, per_leave).unwrap();
    }

    let err = helper.unstake(&alice, per_leave).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::TooManyUnbondingEntries { max: 10 }),
        "expected TooManyUnbondingEntries {{ max: 10 }}, got {root:?}",
    );
}

// ─── FIND-015: expired-reward pruning ──────────────────────────────────────

/// 1) Cap-respect: 5 active streams → adding a 6th fails.
#[test]
fn test_reward_cap_rejects_sixth_when_all_active() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let denoms = ["uusdc", "uusdt", "ubtc", "ueth", "uatom"];
    for d in &denoms {
        helper.mint_coin(&owner, coin(1_000, *d));
        helper.add_reward(&owner, coin(1_000, *d), 10_000).unwrap();
    }

    helper.mint_coin(&owner, coin(1_000, "ufluffy"));
    let err = helper
        .add_reward(&owner, coin(1_000, "ufluffy"), 10_000)
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::TooManyRewardDenoms { max: 5 },
    );
}

/// 2) Lazy prune on AddReward.
#[test]
fn test_reward_slot_freed_after_expiry_and_sixth_admitted() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let short_lived = "ushortlived";
    helper.mint_coin(&owner, coin(1_000, short_lived));
    helper
        .add_reward(&owner, coin(1_000, short_lived), 100)
        .unwrap();

    for d in &["uusdt", "ubtc", "ueth", "uatom"] {
        helper.mint_coin(&owner, coin(1_000, *d));
        helper.add_reward(&owner, coin(1_000, *d), 10_000).unwrap();
    }

    helper.mint_coin(&owner, coin(1_000, "ufluffy"));
    let err = helper
        .add_reward(&owner, coin(1_000, "ufluffy"), 10_000)
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::TooManyRewardDenoms { max: 5 },
    );

    helper.next_block(150);

    helper.mint_coin(&owner, coin(1_000, "ufluffy"));
    helper.add_reward(&owner, coin(1_000, "ufluffy"), 10_000).unwrap();
}

/// 3) FIND-015 LOAD-BEARING: pruned reward still payable via finished index.
#[test]
fn test_pruned_reward_still_payable_via_finished_index() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    let denom = "uusdc";
    helper.mint_coin(&owner, coin(10_000, denom));

    helper.stake(&alice, 10_000).unwrap();
    helper.add_reward(&owner, coin(10_000, denom), 100).unwrap();

    helper.next_block(150);
    let bob = Addr::unchecked("bob");
    helper.give_oro(1, &bob);
    helper.stake(&bob, 1).unwrap();

    let pending = helper.query_pending_rewards(&alice).unwrap();
    let alice_usdc_pending = pending
        .iter()
        .find(|c| c.denom == denom)
        .expect("alice must see USDC pending via the finished-index fallback")
        .amount
        .u128();
    // Alice owns 100% of stake during the whole stream → ~10_000 USDC.
    assert!(
        alice_usdc_pending > 9_990 && alice_usdc_pending <= 10_000,
        "alice USDC pending = {alice_usdc_pending} (expected ~10000)",
    );

    helper.unstake(&alice, 10_000).unwrap();
    helper.next_block(UNBONDING_PERIOD);
    let alice_usdc_before = helper.query_balance(&alice, denom).unwrap().u128();
    helper.claim(&alice).unwrap();
    let alice_usdc_after = helper.query_balance(&alice, denom).unwrap().u128();
    assert_eq!(
        alice_usdc_after - alice_usdc_before,
        alice_usdc_pending,
        "claim must pay out exactly what PendingRewards advertised",
    );
}

/// 4) Re-adding a finished denom resumes from the old final index.
#[test]
fn test_re_adding_finished_denom_resumes_from_final_index() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let denom = "uusdc";
    helper.mint_coin(&owner, coin(20_000, denom));

    helper.add_reward(&owner, coin(10_000, denom), 100).unwrap();
    helper.next_block(150);

    helper.add_reward(&owner, coin(10_000, denom), 100).unwrap();

    let pending = helper.query_pending_rewards(&alice).unwrap();
    let alice_usdc = pending
        .iter()
        .find(|c| c.denom == denom)
        .unwrap()
        .amount
        .u128();
    // Alice owns 100% of stake → ~10_000 USDC from the first stream survives the re-add.
    assert!(
        alice_usdc > 9_990 && alice_usdc <= 10_000,
        "stream A credit must survive the re-add, got {alice_usdc}",
    );
}

// ─── Minimum-stake-amount (per-operation dust floor) ────────────────────────
//
// The contract carries a configurable `minimum_stake_amount` on Config — a
// simple per-operation floor. Both Enter and partial Leave enforce it; FULL
// exits (amount == STAKES[user]) are always allowed regardless of size so
// users can never be trapped above the floor if the owner raises it
// retroactively.

/// Enter below the configured minimum must be rejected with the typed
/// `AmountBelowMinimum` variant. The exact-equal boundary case succeeds.
#[test]
fn test_enter_below_minimum_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new_with_minimum(&owner, Uint128::new(1_000_000)).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000_000, &alice);

    let err = helper.stake(&alice, 500_000).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(
            root,
            ContractError::AmountBelowMinimum { minimum, provided }
                if minimum == Uint128::new(1_000_000) && provided == Uint128::new(500_000)
        ),
        "expected AmountBelowMinimum {{ minimum: 1_000_000, provided: 500_000 }}, got {root:?}",
    );

    // Exactly the minimum succeeds.
    helper.stake(&alice, 1_000_000).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 1_000_000);
}

/// Partial Leave below the minimum is rejected; partial Leave at-or-above the
/// minimum succeeds; the final FULL exit succeeds regardless of size.
#[test]
fn test_leave_below_minimum_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new_with_minimum(&owner, Uint128::new(1_000_000)).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000_000, &alice);
    helper.stake(&alice, 5_000_000).unwrap();

    // Partial Leave of 500_000 < minimum 1_000_000 → rejected (not a full exit).
    let err = helper.unstake(&alice, 500_000).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(
            root,
            ContractError::AmountBelowMinimum { minimum, provided }
                if minimum == Uint128::new(1_000_000) && provided == Uint128::new(500_000)
        ),
        "expected AmountBelowMinimum on sub-floor partial Leave, got {root:?}",
    );

    // Partial Leave of 1_500_000 ≥ minimum → succeeds.
    helper.unstake(&alice, 1_500_000).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 3_500_000);

    // Final FULL exit of the remaining 3_500_000 → always allowed.
    helper.unstake(&alice, 3_500_000).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 0);
}

/// A user holding a balance below the (newly-raised) minimum must still be
/// able to fully exit — full exits bypass the floor. Simulates the operator
/// raising `minimum_stake_amount` after the user already staked.
#[test]
fn test_leave_full_exit_below_minimum_allowed() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new_with_minimum(&owner, Uint128::new(500_000)).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000_000, &alice);
    helper.stake(&alice, 800_000).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 800_000);

    // Owner raises the minimum above Alice's existing balance.
    helper
        .update_config(&owner, None, Some(Uint128::new(1_000_000)))
        .unwrap();

    // A partial Leave would be rejected (800_000 < 1_000_000 and not a full exit
    // since requesting < user_stakes), so verify the partial-floor still bites:
    let err = helper.unstake(&alice, 400_000).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::AmountBelowMinimum { .. }),
        "partial Leave at 400_000 < new minimum 1_000_000 must still reject; got {root:?}",
    );

    // But the FULL exit of the user's entire 800_000 balance is always allowed.
    helper.unstake(&alice, 800_000).unwrap();
    assert_eq!(helper.query_stake_of(&alice).unwrap().u128(), 0);
}

/// Instantiate with `minimum_stake_amount = 0` is rejected up front — the
/// floor must be non-zero or the dust filter does nothing.
#[test]
fn test_instantiate_rejects_zero_minimum() {
    let owner = Addr::unchecked("owner");
    let err = match Helper::try_new_with_minimum(&owner, Uint128::zero()) {
        Ok(_) => panic!("instantiate with minimum=0 must be rejected"),
        Err(e) => e,
    };
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::MinimumStakeAmountIsZero),
        "expected MinimumStakeAmountIsZero, got {root:?}",
    );
}

/// `UpdateConfig` accepts a new `minimum_stake_amount`; subsequent Enter
/// calls are gated by the new floor. Non-owner callers are rejected.
#[test]
fn test_update_config_minimum_stake_amount() {
    let owner = Addr::unchecked("owner");
    let mallory = Addr::unchecked("mallory");
    let mut helper = Helper::new_with_minimum(&owner, Uint128::new(1_000_000)).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000_000, &alice);
    // Sanity: 1_500_000 currently passes (>= 1_000_000).
    helper.stake(&alice, 1_500_000).unwrap();

    // Owner raises the minimum to 2_000_000.
    helper
        .update_config(&owner, None, Some(Uint128::new(2_000_000)))
        .unwrap();
    let cfg = helper.query_config().unwrap();
    assert_eq!(cfg.minimum_stake_amount, Uint128::new(2_000_000));

    // Now an Enter of 1_500_000 < 2_000_000 is rejected.
    let err = helper.stake(&alice, 1_500_000).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::AmountBelowMinimum { .. }),
        "expected AmountBelowMinimum at new floor, got {root:?}",
    );

    // Enter of exactly 2_000_000 succeeds.
    helper.stake(&alice, 2_000_000).unwrap();

    // Non-owner update_config is rejected before any state mutation.
    let err = helper
        .update_config(&mallory, None, Some(Uint128::new(3_000_000)))
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert_eq!(root, ContractError::Unauthorized {});

    // And the floor stays where the owner left it.
    let cfg = helper.query_config().unwrap();
    assert_eq!(cfg.minimum_stake_amount, Uint128::new(2_000_000));

    // Zero is also rejected on update.
    let err = helper
        .update_config(&owner, None, Some(Uint128::zero()))
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::MinimumStakeAmountIsZero),
        "expected MinimumStakeAmountIsZero on update to zero, got {root:?}",
    );
}

/// 5) Withdraw-excess guard must reject finished-stream denoms too.
#[test]
fn test_withdraw_excess_blocked_for_finished_reward_denom() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let denom = "uusdc";
    helper.mint_coin(&owner, coin(1_000, denom));
    helper.add_reward(&owner, coin(1_000, denom), 100).unwrap();

    helper.next_block(150);
    let bob = Addr::unchecked("bob");
    helper.give_oro(1, &bob);
    helper.stake(&bob, 1).unwrap();

    let err = helper
        .withdraw_excess_tokens(&owner, denom, None)
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::DenomInRewardStream {
            denom: denom.to_string()
        },
    );
}

// ─── Orphaned rewards (mirrors incentives pattern) ───────────────────────────

/// When a reward stream emits during a `TOTAL_STAKES == 0` window the
/// emission must NOT be silently dropped — it must accrue to
/// `ORPHANED_REWARDS[denom]` so the owner can sweep it later, while the
/// first staker to arrive after the orphan window only sees pending accrual
/// from the post-arrival sub-window.
#[test]
fn test_no_staker_emission_orphaned() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // Stream uzig: 60_000 over 60s → rps = 1_000. No stakers yet.
    let denom = "uzig";
    helper.mint_coin(&owner, coin(60_000, denom));
    helper.add_reward(&owner, coin(60_000, denom), 60).unwrap();

    // 30s elapse with TOTAL_STAKES == 0 — that 30_000 uzig must orphan.
    helper.next_block(30);

    // First user Enters now. The orphan write happens inside this call's
    // `update_rewards` (the previous next_block doesn't trigger any contract
    // execution on its own).
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let orphan = helper.query_orphaned_rewards(denom);
    assert_eq!(
        orphan.u128(),
        30_000,
        "30s * rps(1_000) must orphan when no staker present at the time of stake-in",
    );

    // Second half of the stream elapses with alice as the sole staker.
    helper.next_block(30);

    // Alice unstakes everything, waits, claims. She must receive ONLY the
    // post-arrival 30_000 — NOT the full 60_000 (the first 30_000 is orphaned).
    helper.unstake(&alice, 10_000).unwrap();
    helper.next_block(UNBONDING_PERIOD);

    let before = helper.query_balance(&alice, denom).unwrap().u128();
    helper.claim(&alice).unwrap();
    let after = helper.query_balance(&alice, denom).unwrap().u128();
    let received = after - before;

    assert!(
        (29_999..=30_000).contains(&received),
        "first depositor must only receive post-orphan share (~30_000), got {received}",
    );

    // Orphan bucket is unchanged by alice's claim — only owner sweep clears it.
    let orphan_after = helper.query_orphaned_rewards(denom);
    assert_eq!(
        orphan_after.u128(),
        30_000,
        "orphan bucket must be preserved until owner sweeps via ClaimOrphanedRewards",
    );
}

/// `ClaimOrphanedRewards` is owner-only and clears the bucket on success.
#[test]
fn test_claim_orphaned_rewards_owner_only() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // Stream with no staker for the full window → all emission orphans.
    let denom = "uzig";
    helper.mint_coin(&owner, coin(50_000, denom));
    helper.add_reward(&owner, coin(50_000, denom), 50).unwrap();

    // Advance past the end of the stream, then trigger any contract execute
    // that calls update_rewards. Use a tiny AddReward to flush the orphan
    // accrual into storage. (next_block alone does nothing — no execution.)
    helper.next_block(60);

    // Re-add the same denom with a fresh duration; this calls
    // update_rewards which lazy-prunes the finished stream AND writes the
    // orphan bucket for the no-staker window.
    helper.mint_coin(&owner, coin(1_000, denom));
    helper.add_reward(&owner, coin(1_000, denom), 100).unwrap();

    let orphan = helper.query_orphaned_rewards(denom);
    assert!(
        orphan.u128() >= 49_999,
        "expected ~50_000 orphaned, got {}",
        orphan.u128(),
    );

    // Non-owner cannot sweep.
    let mallory = Addr::unchecked("mallory");
    let err = helper.claim_orphaned_rewards(&mallory, None).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {},
    );

    // Owner sweep succeeds and clears the bucket.
    let owner_before = helper.query_balance(&owner, denom).unwrap().u128();
    helper.claim_orphaned_rewards(&owner, None).unwrap();
    let owner_after = helper.query_balance(&owner, denom).unwrap().u128();

    let received = owner_after - owner_before;
    assert!(
        received >= 49_999,
        "owner must receive the orphan bucket, got {received}",
    );
    assert_eq!(
        helper.query_orphaned_rewards(denom).u128(),
        0,
        "bucket must be cleared after sweep",
    );
}

/// Multi-denom sweep emits a single BankMsg::Send carrying every denom.
/// Asserted via bank balance deltas on the recipient for each denom.
#[test]
fn test_claim_orphaned_rewards_multi_denom() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // Two reward streams, both emit during a no-staker window.
    let d1 = "uzig";
    let d2 = "uother";
    helper.mint_coin(&owner, coin(10_000, d1));
    helper.mint_coin(&owner, coin(20_000, d2));
    helper.add_reward(&owner, coin(10_000, d1), 100).unwrap();
    helper.add_reward(&owner, coin(20_000, d2), 100).unwrap();

    helper.next_block(50); // half of each stream elapses w/ TOTAL_STAKES == 0

    // Trigger a state-touching call so update_rewards commits both orphan
    // rows. The cheapest is a third AddReward (any denom is fine).
    helper.mint_coin(&owner, coin(1, "trigger"));
    helper.add_reward(&owner, coin(1, "trigger"), 1).unwrap();

    let orphan_d1 = helper.query_orphaned_rewards(d1).u128();
    let orphan_d2 = helper.query_orphaned_rewards(d2).u128();
    assert!(orphan_d1 >= 4_999, "uzig orphan {orphan_d1}");
    assert!(orphan_d2 >= 9_999, "uother orphan {orphan_d2}");

    // Sweep to a fresh recipient.
    let treasury = Addr::unchecked("treasury");
    let before_d1 = helper.query_balance(&treasury, d1).unwrap().u128();
    let before_d2 = helper.query_balance(&treasury, d2).unwrap().u128();

    helper
        .claim_orphaned_rewards(&owner, Some(&treasury))
        .unwrap();

    let after_d1 = helper.query_balance(&treasury, d1).unwrap().u128();
    let after_d2 = helper.query_balance(&treasury, d2).unwrap().u128();

    assert_eq!(after_d1 - before_d1, orphan_d1, "uzig delta must equal bucket");
    assert_eq!(after_d2 - before_d2, orphan_d2, "uother delta must equal bucket");

    // Both buckets cleared.
    assert_eq!(helper.query_orphaned_rewards(d1).u128(), 0);
    assert_eq!(helper.query_orphaned_rewards(d2).u128(), 0);
}

/// An empty bucket must surface as `NoOrphanedRewards` rather than emitting
/// an empty BankMsg::Send (Cosmos SDK rejects zero-coin sends anyway).
#[test]
fn test_claim_orphaned_rewards_empty_bucket_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let err = helper.claim_orphaned_rewards(&owner, None).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::NoOrphanedRewards,
    );
}

/// First depositor arriving after a no-staker window MUST NOT receive
/// retroactive credit for the orphan period.
#[test]
fn test_first_depositor_after_no_staker_no_retroactive() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let denom = "uzig";
    helper.mint_coin(&owner, coin(60_000, denom));
    helper.add_reward(&owner, coin(60_000, denom), 60).unwrap();

    // 30s with no stakers.
    helper.next_block(30);

    // First user enters at T+30.
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    // Stream finishes (30s more with alice as the sole staker).
    helper.next_block(30);

    // Force a sync of alice's pending by querying.
    let pending = helper.query_pending_rewards(&alice).unwrap();
    let alice_zig = pending
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    assert!(
        (29_999..=30_000).contains(&alice_zig),
        "alice must only accrue post-arrival share (~30_000), got {alice_zig}",
    );

    // Confirm the orphan bucket caught the first-half emission.
    let orphan = helper.query_orphaned_rewards(denom).u128();
    assert_eq!(orphan, 30_000, "first half must be orphaned, got {orphan}");
}

/// A partial no-staker window between two staking periods must orphan only
/// the emission from the empty gap, leaving the surrounding stakers' payouts
/// proportional to their actual exposure.
#[test]
fn test_partial_no_staker_window() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let denom = "uzig";
    // 60_000 over 60s → rps = 1_000.
    helper.mint_coin(&owner, coin(60_000, denom));
    helper.add_reward(&owner, coin(60_000, denom), 60).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 100).unwrap();

    // T0 → T+20 with alice as sole staker; she accrues 20_000.
    helper.next_block(20);

    // Alice fully unbonds → TOTAL_STAKES becomes 0.
    helper.unstake(&alice, 100).unwrap();

    // T+20 → T+40 with no stakers → must orphan 20_000.
    helper.next_block(20);

    // Bob enters at T+40.
    let bob = Addr::unchecked("bob");
    helper.give_oro(10_000, &bob);
    helper.stake(&bob, 100).unwrap();

    // T+40 → T+60 with bob as sole staker; he should accrue ~20_000.
    helper.next_block(20);

    // Orphan bucket should reflect only the middle gap.
    let orphan = helper.query_orphaned_rewards(denom).u128();
    assert!(
        (19_999..=20_000).contains(&orphan),
        "middle no-staker window must orphan ~20_000, got {orphan}",
    );

    // Alice's claim chain: matured unbonding entry pays principal + the
    // 20_000 reward coins accrued before she left.
    helper.next_block(UNBONDING_PERIOD);
    let alice_zig_before = helper.query_balance(&alice, denom).unwrap().u128();
    helper.claim(&alice).unwrap();
    let alice_zig_after = helper.query_balance(&alice, denom).unwrap().u128();
    let alice_received = alice_zig_after - alice_zig_before;
    assert!(
        (19_999..=20_000).contains(&alice_received),
        "alice must receive her pre-leave window (~20_000), got {alice_received}",
    );

    // Bob unbonds and claims his post-arrival share.
    helper.unstake(&bob, 100).unwrap();
    helper.next_block(UNBONDING_PERIOD);
    let bob_zig_before = helper.query_balance(&bob, denom).unwrap().u128();
    helper.claim(&bob).unwrap();
    let bob_zig_after = helper.query_balance(&bob, denom).unwrap().u128();
    let bob_received = bob_zig_after - bob_zig_before;
    assert!(
        (19_999..=20_000).contains(&bob_received),
        "bob must receive his post-arrival share (~20_000), got {bob_received}",
    );

    // Total accounting: alice + bob + orphan ≈ 60_000 (modulo ≤2 wei floor).
    let total = alice_received + bob_received + orphan;
    assert!(
        (59_998..=60_000).contains(&total),
        "total emissions distributed + orphaned must equal funded stream, got {total}",
    );
}
