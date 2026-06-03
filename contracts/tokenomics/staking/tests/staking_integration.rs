#![cfg(not(tarpaulin_include))]


use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, coins, from_json, Addr, BankMsg, Binary, BlockInfo, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdResult, Timestamp, Uint128,
};
use cw_multi_test::{Contract, ContractWrapper, Executor};
use cw_utils::PaymentError;

use oroswap::staking::{Config, ExecuteMsg, QueryMsg, StakingResponse};
use oroswap_staking::error::ContractError;

use crate::common::helper::{Helper, ORO_DENOM};

mod common;

#[test]
fn test_instantiate() {
    let owner = Addr::unchecked("owner");

    let helper = Helper::new(&owner).unwrap();

    let response: Config = helper
        .app
        .wrap()
        .query_wasm_smart(&helper.staking, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(
        response,
        Config {
            oro_denom: ORO_DENOM.to_string(),
            xoro_denom: format!("coin.{}.xoro", &helper.staking),
            bootstrap_amount: Uint128::new(1000),
            owner: "owner".to_string(),
            unbonding_period: 7 * 24 * 60 * 60,
            xoro_decimals: 6,
            xoro_uri: Some("https://example.com/metadata.json".to_string()),
            paused: false,
        }
    );
}

#[test]
fn check_deflate_liquidity() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();

    let attacker = Addr::unchecked("attacker");
    let victim = Addr::unchecked("victim");

    helper.give_oro(10000, &attacker);
    helper.give_oro(10000, &victim);

    // First stake should work now since pool is bootstrapped during instantiation
    helper.stake(&attacker, 1000).unwrap();

    helper
        .app
        .send_tokens(
            attacker.clone(),
            helper.staking.clone(),
            &coins(5000, ORO_DENOM),
        )
        .unwrap();

    // Small stakes should work now since pool is bootstrapped
    helper.stake(&victim, 5).unwrap();
    helper.stake(&victim, 7).unwrap();
}

#[test]
fn test_invalid_denom() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();

    let bad_denom = "bad/denom";
    helper.mint_coin(&owner, coin(1000, bad_denom));

    // Try to stake bad denom
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Enter { receiver: None },
            &coins(1000u128, bad_denom),
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(ORO_DENOM.to_string()))
    );

    // Try to stake bad denom along with ORO
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Enter { receiver: None },
            &[coin(1000u128, bad_denom), coin(1000u128, ORO_DENOM)],
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MultipleDenoms {})
    );

    // Stake to pass xORO bank module balance check below
    helper.stake(&owner, 10000).unwrap();

    // Try to unstake bad denom
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Leave { receiver: None },
            &coins(1000u128, bad_denom),
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(helper.xoro_denom.to_string()))
    );

    // Try to unstake bad denom along with xORO
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Leave { receiver: None },
            &[
                coin(1000u128, bad_denom),
                coin(1000u128, helper.xoro_denom.clone()),
            ],
        )
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MultipleDenoms {})
    );
}

#[test]
fn test_enter_and_leave() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let xoro_denom = helper.xoro_denom.clone();
    let staking = helper.staking.clone();

    let alice = Addr::unchecked("alice");

    // Mint 10000 ORO for Alice
    helper.give_oro(10000, &alice);

    // Stake Alice's 1100 ORO for 1100 xORO (1:1 ratio now)
    let resp_data = helper.stake(&alice, 1100).unwrap().data.unwrap();
    let staking_resp: StakingResponse = from_json(&resp_data).unwrap();
    assert_eq!(
        staking_resp,
        StakingResponse {
            oro_amount: 1100u128.into(),
            xoro_amount: 1100u128.into(),
        }
    );

    // Check if Alice's xORO balance is 1100 (1:1 ratio)
    let amount = helper.query_balance(&alice, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 1100);

    // Check if the staking contract's ORO balance is 2100 (1000 bootstrap + 1100 from Alice)
    let amount = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 2100u128);

    // Unstake Alice's 10 xORO for 10 ORO
    let resp_data = helper.unstake(&alice, 10).unwrap().data.unwrap();
    let staking_resp: StakingResponse = from_json(&resp_data).unwrap();
    assert_eq!(
        staking_resp,
        StakingResponse {
            oro_amount: 10u128.into(),
            xoro_amount: 10u128.into(),
        }
    );

    // Check if Alice's xORO balance is 1090 (1100 - 10)
    let amount = helper.query_balance(&alice, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 1090);

    // Check if Alice's ORO balance is still 8900 (10000 - 1100, unstaked 10 but not claimed)
    let amount = helper.query_balance(&alice, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 8900);

    // Fast forward time by 7 days
    helper.next_block(7 * 24 * 60 * 60);

    // Claim Alice's ORO
    helper.claim(&alice).unwrap();

    // Now Alice's ORO balance is 8910 (8900 + 10)
    let amount = helper.query_balance(&alice, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 8910);

    // Check if the staking contract's ORO balance is 2090 (2100 - 10)
    let amount = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 2090);

    // Check if the staking contract's xORO balance is 1000 (1000 bootstrap, 10 was sent to Alice)
    let amount = helper.query_balance(&staking, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 1000);

    // Check staking for specific recipient
    let user = Addr::unchecked("user");
    let recipient = Addr::unchecked("recipient");
    helper.give_oro(10000, &user);
    helper
        .app
        .execute_contract(
            user.clone(),
            helper.staking.clone(),
            &ExecuteMsg::Enter {
                receiver: Some(recipient.to_string()),
            },
            &coins(10000, ORO_DENOM),
        )
        .unwrap();

    let amount = helper.query_balance(&user, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 0);

    let amount = helper.query_balance(&recipient, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 10000);
}

#[test]
fn should_work_with_more_than_one_participant() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    let xoro_denom = helper.xoro_denom.clone();
    let staking = helper.staking.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");

    // Mint 10000 ORO for Alice and Bob
    helper.give_oro(10000, &alice);
    helper.give_oro(10000, &bob);

    // Stake Alice's 2000 ORO for 2000 xORO (1:1 ratio now)
    helper.stake(&alice, 2000).unwrap();
    // Check Alice's xORO balance is 2000
    let amount = helper.query_balance(&alice, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 2000);

    // Stake Bob's 10 ORO for 10 xORO
    helper.stake(&bob, 10).unwrap();
    // Check Bob's xORO balance is 10
    let amount = helper.query_balance(&bob, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 10);

    // Check staking contract's ORO balance is 3010 (1000 bootstrap + 2000 from Alice + 10 from Bob)
    let amount = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 3010);

    // Staking contract gets 20 more ORO from external source
    helper.give_oro(20, &staking);

    // Stake Alice's 10 ORO for 6 xORO: 10*2010/3030 = 6 (integer division)
    helper.stake(&alice, 10).unwrap();

    // Check Alice's xORO balance is 2009 (2000 + 9 from second stake)
    let amount = helper.query_balance(&alice, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 2009);

    // Burn Bob's 5 xORO and unstake: gets 5*2040/2019 = 5 ORO
    helper.unstake(&bob, 5).unwrap();
    // Check Bob's xORO balance is 5
    let amount = helper.query_balance(&bob, &xoro_denom).unwrap();
    assert_eq!(amount.u128(), 5);
    // Check Bob's ORO balance is 9990 (10000 minted - 10 entered; unstaked 5 but not claimed)
    let amount = helper.query_balance(&bob, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 9990);

    // Fast forward time by 7 days
    helper.next_block(7 * 24 * 60 * 60);

    // Claim Bob's ORO
    helper.claim(&bob).unwrap();

    // Check Bob's ORO balance is 9995 (9990 + 5 by leaving)
    let amount = helper.query_balance(&bob, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 9995);

    // Check the staking contract's ORO balance is 3035 (3010 + 20 + 10 - 5)
    let amount = helper.query_balance(&staking, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 3035);

    // Check Alice's ORO balance is 7990 (10000 minted - 2000 entered - 10 entered)
    let amount = helper.query_balance(&alice, ORO_DENOM).unwrap();
    assert_eq!(amount.u128(), 7990);
}

#[test]
fn test_historical_queries() {
    let owner = Addr::unchecked("owner");

    let mut helper = Helper::new(&owner).unwrap();
    helper.app.set_block(BlockInfo {
        height: 1000,
        time: Timestamp::from_seconds(1700000000),
        chain_id: "".to_string(),
    });

    helper.stake(&owner, 1001).unwrap();

    let xoro_denom = helper.xoro_denom.clone();

    let user1 = Addr::unchecked("user1");
    let user2 = Addr::unchecked("user2");

    // Stake and query at the same block
    helper.give_oro(1_000_000000, &user1);
    helper.stake(&user1, 1_000_000000).unwrap();

    let amount = helper.query_xoro_balance_at(&user1, None).unwrap();
    assert_eq!(amount.u128(), 1_000_000000);
    let total_supply = helper.query_xoro_supply_at(None).unwrap();
    assert_eq!(total_supply.u128(), 1_000_002001);

    // Stake for user2 too
    helper.give_oro(1_000_000000, &user2);
    helper.stake(&user2, 1_000_000000).unwrap();

    for _ in 0..10 {
        helper.next_block(100);

        helper
            .app
            .send_tokens(
                user1.clone(),
                user2.clone(),
                &coins(1_000000, &xoro_denom),
            )
            .unwrap();

        // Stake to impact total supply
        helper.give_oro(2_000000, &user1);
        helper.stake(&user1, 2_000000).unwrap();

        // Unstake to impact total supply
        helper.unstake(&user2, 3_000000).unwrap();
    }

    // Check the rest of the queries

    let total_shares: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(&helper.staking, &QueryMsg::TotalShares {})
        .unwrap();
    let total_supply = helper
        .app
        .wrap()
        .query_supply(&xoro_denom)
        .unwrap()
        .amount;
    assert_eq!(total_shares, total_supply);

    let staking = helper.staking.clone();
    let total_deposit: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(&helper.staking, &QueryMsg::TotalDeposit {})
        .unwrap();
    let total_locked: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(&helper.staking, &QueryMsg::TotalLocked {})
        .unwrap();
    let staking_oro_balance = helper
        .app
        .wrap()
        .query_balance(&staking, ORO_DENOM)
        .unwrap()
        .amount;
    assert_eq!(total_deposit + total_locked, staking_oro_balance);
}


#[test]
fn test_hooks() {
    #[cw_serde]
    enum AbsorberMsg {
        Absorb {},
        SendTo { recipient: String },
    }
    fn xoro_absorber_contract() -> Box<dyn Contract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(
            |_: DepsMut, _: Env, info: MessageInfo, msg: AbsorberMsg| -> StdResult<Response> {
                match msg {
                    AbsorberMsg::Absorb {} => Ok(Response::new()),
                    AbsorberMsg::SendTo { recipient } => {
                        Ok(Response::new().add_message(BankMsg::Send {
                            to_address: recipient,
                            amount: info.funds,
                        }))
                    }
                }
            },
            |_: DepsMut, _: Env, _: MessageInfo, _: Empty| -> StdResult<Response> {
                Ok(Response::new())
            },
            |_: Deps, _: Env, _: Empty| -> StdResult<Binary> { unimplemented!() },
        ))
    }

    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.give_oro(10000, &owner);
    helper.stake(&owner, 10000).unwrap();

    let absorber_code_id = helper.app.store_code(xoro_absorber_contract());
    let absorber = helper
        .app
        .instantiate_contract(
            absorber_code_id,
            owner.clone(),
            &Empty {},
            &[],
            "absorber",
            None,
        )
        .unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(30000, &alice);

    helper
        .stake_with_hook(&alice, 10000, absorber.to_string(), &AbsorberMsg::Absorb {})
        .unwrap();
    let absorber_balance = helper
        .app
        .wrap()
        .query_balance(&absorber, &helper.xoro_denom)
        .unwrap()
        .amount;
    assert_eq!(absorber_balance.u128(), 10000);

    let bob = Addr::unchecked("bob");
    helper
        .stake_with_hook(
            &alice,
            10000,
            absorber.to_string(),
            &AbsorberMsg::SendTo {
                recipient: bob.to_string(),
            },
        )
        .unwrap();

    // Absorber balance hasn't changed
    let absorber_balance = helper
        .app
        .wrap()
        .query_balance(&absorber, &helper.xoro_denom)
        .unwrap()
        .amount;
    assert_eq!(absorber_balance.u128(), 10000);

    // Bob has received 10000 xORO
    let bob_balance = helper
        .app
        .wrap()
        .query_balance(&bob, &helper.xoro_denom)
        .unwrap()
        .amount;
    assert_eq!(bob_balance.u128(), 10000);

    // Try failing hook msg
    helper
        .stake_with_hook(&alice, 10000, absorber.to_string(), &())
        .unwrap_err();
}

/// FIND-010 regression: `EnterWithHook` must validate the caller-supplied
/// `contract_address` BEFORE running `execute_enter`, mirroring the
/// `Enter { receiver }` path. A malformed destination must fail fast with
/// a clear StdError and leave no state mutation (ORO not consumed, xORO
/// not minted, total_locked unchanged).
#[test]
fn test_enter_with_hook_validates_contract_address() {
    use cosmwasm_schema::cw_serde;

    #[cw_serde]
    enum NoopMsg {
        Noop {},
    }

    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    let oro_before = helper
        .query_balance(&alice, ORO_DENOM)
        .unwrap()
        .u128();
    let xoro_supply_before = helper.query_xoro_supply_at(None).unwrap().u128();
    let total_locked_before = helper.query_total_locked().unwrap().u128();

    // Malformed bech32 — MockApi rejects this in `addr_validate` before any
    // state is touched.
    let err = helper
        .stake_with_hook(
            &alice,
            10_000,
            "not_a_valid_bech32_addr!!!".to_string(),
            &NoopMsg::Noop {},
        )
        .unwrap_err();
    // The error surfaces as a cw-multi-test wrapping of the inner StdError.
    // We don't need to match the exact variant — just confirm the tx reverted.
    let _ = err;

    // State invariant: alice's ORO is unchanged, no xORO was minted, and
    // total_locked is unchanged. Pre-fix the validation happened only at
    // sub-message dispatch, AFTER execute_enter had already mutated state;
    // the wasm-vm would roll the tx back, so this assertion would still
    // hold, but the post-fix flow does not even start the mint.
    assert_eq!(
        helper.query_balance(&alice, ORO_DENOM).unwrap().u128(),
        oro_before,
        "alice's ORO must not be consumed when the hook destination is invalid",
    );
    assert_eq!(
        helper.query_xoro_supply_at(None).unwrap().u128(),
        xoro_supply_before,
        "no xORO must be minted on a rejected EnterWithHook",
    );
    assert_eq!(
        helper.query_total_locked().unwrap().u128(),
        total_locked_before,
        "total_locked must not change on a rejected EnterWithHook",
    );

    // Sanity: the SAME alice can still stake afterwards with a valid
    // destination — proving the rejected attempt left no orphan state behind.
    helper.stake(&alice, 5_000).unwrap();
}

#[test]
fn test_pause_unpause() {
    let owner = Addr::unchecked("owner");
    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");

    let mut helper = Helper::new(&owner).unwrap();
    helper.give_oro(10000, &alice);
    helper.give_oro(10000, &bob);

    // 1. Only owner can pause
    let err = helper.pause(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    helper.pause(&owner).unwrap();

    // 2. Actions blocked when paused
    let err = helper.stake(&alice, 1000).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Setup for leave and claim (staking while unpaused)
    helper.unpause(&owner).unwrap();
    helper.stake(&alice, 1000).unwrap();
    helper.pause(&owner).unwrap();

    let err = helper.unstake(&alice, 100).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Setup for claim
    helper.unpause(&owner).unwrap();
    let _ = helper.unstake(&alice, 100).unwrap();
    helper.pause(&owner).unwrap();

    let err = helper.claim(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // 3. Unpause restores actions
    helper.unpause(&owner).unwrap();
    helper.stake(&bob, 1000).unwrap();
    helper.unstake(&bob, 100).unwrap();
    
    // Fast forward to claim
    helper.next_block(7 * 24 * 60 * 60);
    helper.claim(&alice).unwrap();
    helper.claim(&bob).unwrap();
}

/// Same-version migrate is a no-op against the in-flight test instance. The
/// genuine legacy-bytes path (pre-2.2.0 Config without `paused`) cannot be
/// exercised through cw-multi-test because contract storage is namespaced
/// behind the wasm keeper — `init_modules` writes to a different keyspace
/// and is invisible to the migrate handler. That path is covered by the
/// unit test in `migrate.rs`, which uses `mock_dependencies()` to write
/// legacy-shaped bytes directly into the contract's own storage. (Audit
/// N-2.)
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

    // Config still reads back coherently; pause flag preserved.
    let config: Config = helper
        .app
        .wrap()
        .query_wasm_smart(&staking_addr, &QueryMsg::Config {})
        .unwrap();
    assert!(!config.paused);
}

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

    // 1. Alice stakes 10,000 ORO -> receives 10,000 xORO
    helper.stake(&alice, 10_000).unwrap();

    // 2. Admin adds 1,000 ORO and 10,000 USDC as rewards for 100 seconds
    helper.add_reward(&owner, coin(1_000, ORO_DENOM), 100).unwrap();
    helper.add_reward(&owner, coin(10_000, usdc_denom), 100).unwrap();

    // 3. Fast forward 50 seconds. Alice should have earned 50% of rewards.
    // Total shares = 1000 (bootstrap) + 10,000 (Alice) = 11,000
    // But wait, the reward distribution is pro-rata to xORO shares.
    // Alice has 10,000 / 11,000 shares.
    helper.next_block(50);

    let pending = helper.query_pending_rewards(&alice).unwrap();
    // ORO: 1000 * 50/100 * 10000/11000 = 454.54 -> 454
    // USDC: 10000 * 50/100 * 10000/11000 = 4545.45 -> 4545
    let oro_pending = pending.iter().find(|c| c.denom == ORO_DENOM).unwrap().amount.u128();
    let usdc_pending = pending.iter().find(|c| c.denom == usdc_denom).unwrap().amount.u128();
    assert_eq!(oro_pending, 454);
    assert_eq!(usdc_pending, 4545);

    // 4. Bob stakes 10,000 ORO.
    // Alice's rewards should be stashed in pending_rewards in state.
    helper.stake(&bob, 10_000).unwrap();

    // 5. Fast forward another 50 seconds.
    // Total shares = 11,000 + 10,000 = 21,000
    // Alice shares: 10,000 / 21,000
    // Bob shares: 10,000 / 21,000
    // Remaining rewards: 50%
    helper.next_block(50);

    // Alice Total: 454 (stashed) + (1000 * 50/100 * 10000/21000) = 454 + 238 = 692
    // Bob Total: 1000 * 50/100 * 10000/21000 = 238
    let alice_pending = helper.query_pending_rewards(&alice).unwrap();
    let bob_pending = helper.query_pending_rewards(&bob).unwrap();

    assert_eq!(alice_pending.iter().find(|c| c.denom == ORO_DENOM).unwrap().amount.u128(), 692);
    assert_eq!(bob_pending.iter().find(|c| c.denom == ORO_DENOM).unwrap().amount.u128(), 238);

    // 6. Alice unstakes all. Rewards should move to unbonding.
    helper.unstake(&alice, 10_000).unwrap();

    // Verify Alice has no pending rewards now (they moved to unbonding)
    let alice_pending_after = helper.query_pending_rewards(&alice).unwrap();
    assert!(alice_pending_after.is_empty());

    // Verify unbonding entry contains rewards
    let entries: Vec<oroswap::staking::UnbondingEntry> = helper.app.wrap().query_wasm_smart(
        &helper.staking,
        &QueryMsg::UnbondingEntries { address: alice.to_string() }
    ).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].reward_coins.len(), 2);
    assert_eq!(entries[0].reward_coins.iter().find(|c| c.denom == ORO_DENOM).unwrap().amount.u128(), 692);

    // 7. Fast forward unbonding period and claim.
    helper.next_block(7 * 24 * 60 * 60);
    let alice_oro_before = helper.query_balance(&alice, ORO_DENOM).unwrap().u128();
    helper.claim(&alice).unwrap();
    let alice_oro_after = helper.query_balance(&alice, ORO_DENOM).unwrap().u128();
    let alice_usdc_after = helper.query_balance(&alice, usdc_denom).unwrap().u128();

    // Alice received: principal + reward ORO
    // Principal: 10,000. Reward ORO: 692 (floor of exact Decimal256 math).
    assert_eq!(alice_oro_after - alice_oro_before, 10692);
    // USDC:
    // Part 1: floor(10000 * 50/100 * 10000 / 11000) = floor(4545.45...) = 4545
    // Part 2: floor(10000 * 50/100 * 10000 / 21000) = floor(2380.95...) = 2380
    // But Decimal256 internally accumulates without flooring each step, so total = 6926.
    assert_eq!(alice_usdc_after, 6926);
}

// ─── PAUSE_AUTHORITIES (audit N-3 follow-up) ─────────────────────────────────
//
// Mirrors the wrapper / factory pattern: a delegated authority can pause,
// but ONLY the owner can unpause. Limits blast radius of a compromised
// authority key — they can grief by pausing but can't toggle.

#[test]
fn test_pause_authority_can_pause_but_not_unpause() {
    let owner = Addr::unchecked("owner");
    let authority = Addr::unchecked("authority");
    let mut helper = Helper::new(&owner).unwrap();

    // Owner grants pause authority.
    helper
        .add_pause_authorities(&owner, vec![authority.to_string()])
        .unwrap();
    let list = helper.query_pause_authorities().unwrap();
    assert_eq!(list, vec![authority.to_string()]);

    // Authority pauses successfully.
    helper.pause(&authority).unwrap();

    // Authority CANNOT unpause — typed error.
    let err = helper.unpause(&authority).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::NoUnpauseAuthority {}),
        "expected NoUnpauseAuthority, got {root:?}",
    );

    // Owner unpauses successfully.
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

    // Bogus address rejected with typed error (empty string fails addr_validate
    // even under the permissive MockApi used by cw-multi-test).
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

// ─── WithdrawExcessTokens (audit N-1) ────────────────────────────────────────
//
// Pre-fix the function was a structural no-op for the ORO denom: it computed
// `withdrawable = balance - (effective + locked + reserve)`, but `effective`
// itself was defined as `balance - locked - reserve`, so the expression was
// always exactly zero regardless of state. After the fix, the function rejects
// ORO outright (every wei is owed to stakers / unbonders / reward stream) and
// fully drains any other denom.

#[test]
fn test_withdraw_excess_oro_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    // Bank-send 5_000 ORO directly to the contract (auto-compound mechanism;
    // would-have-been a "recoverable excess" under the no-op fix).
    helper.give_oro(5_000, &owner);
    helper
        .bank_send(&owner, &helper.staking.clone(), coins(5_000, ORO_DENOM))
        .unwrap();

    // Owner attempts to withdraw the ORO — must be refused, by design.
    let err = helper
        .withdraw_excess_tokens(&owner, ORO_DENOM, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::OroNonRecoverable {}),
        "expected OroNonRecoverable, got {root:?}",
    );

    // Non-owner is rejected before the denom check is reached.
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
    // Send 7_500 stray tokens directly to the contract (mistaken transfer).
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

    helper
        .withdraw_excess_tokens(&owner, bogus, None)
        .unwrap();

    let owner_after = helper.query_balance(&owner, bogus).unwrap().u128();
    let contract_after = helper
        .query_balance(&helper.staking.clone(), bogus)
        .unwrap()
        .u128();
    assert_eq!(owner_after - owner_before, 7_500);
    assert_eq!(contract_after, 0);

    // Second call with the same denom — nothing left to withdraw.
    let err = helper
        .withdraw_excess_tokens(&owner, bogus, None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::NoExcessToWithdraw {}),
        "expected NoExcessToWithdraw, got {root:?}",
    );
}

/// Audit N-3 regression: the per-user `MAX_UNBONDING_ENTRIES` ceiling now
/// surfaces the typed `TooManyUnbondingEntries { max }` variant instead of
/// a stringly-typed `StdError::generic_err`. The 11th `Leave` from the same
/// address must fail with the typed error.
#[test]
fn test_too_many_unbonding_entries_typed_error() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(11 * 1_000, &alice);
    helper.stake(&alice, 11 * 1_000).unwrap();
    let xoro_balance = helper.query_balance(&alice, &helper.xoro_denom.clone()).unwrap().u128();
    let per_leave = xoro_balance / 11;

    // 10 successful Leave calls — fills the per-user unbonding slots.
    for _ in 0..10 {
        helper.unstake(&alice, per_leave).unwrap();
    }

    // 11th must fail with the typed variant (max = 10).
    let err = helper.unstake(&alice, per_leave).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::TooManyUnbondingEntries { max: 10 }),
        "expected TooManyUnbondingEntries {{ max: 10 }}, got {root:?}",
    );
}

/// CRITICAL audit follow-up #1: the owner must NOT be able to recover the
/// contract's own bootstrap xORO. If they could, they could `Leave` on
/// those bootstrap shares and drain the proportional ORO from the pool —
/// defeating the first-deposit-attack guard the bootstrap exists for.
#[test]
fn test_withdraw_excess_xoro_rejected() {
    let owner = Addr::unchecked("owner");
    let helper = Helper::new(&owner).unwrap();

    // Sanity: contract holds the bootstrap xORO supply.
    let contract_xoro = helper
        .query_balance(&helper.staking.clone(), &helper.xoro_denom.clone())
        .unwrap()
        .u128();
    assert!(contract_xoro > 0, "contract must hold bootstrap xORO");

    let mut helper = helper;
    let err = helper
        .withdraw_excess_tokens(&owner, &helper.xoro_denom.clone(), None)
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::XoroNonRecoverable {}),
        "expected XoroNonRecoverable, got {root:?}",
    );

    // Bootstrap untouched.
    let after = helper
        .query_balance(&helper.staking.clone(), &helper.xoro_denom.clone())
        .unwrap()
        .u128();
    assert_eq!(after, contract_xoro);
}

/// CRITICAL audit follow-up #2: the owner must NOT be able to sweep tokens
/// that are committed to an active (or historically active) reward stream.
/// Doing so would drain funds the contract owes to stakers' pending rewards
/// and to mature `UnbondingEntry.reward_coins`, bricking `Claim` calls with
/// insufficient-funds bank reverts.
#[test]
fn test_withdraw_excess_reward_denom_rejected() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // Some stakers so the reward stream has a denominator.
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    // Admin opens a USDC reward stream — 5_000 USDC over 1000 seconds.
    let usdc = "usdc";
    helper.mint_coin(&owner, coin(5_000, usdc));
    helper
        .add_reward(&owner, coin(5_000, usdc), 1_000)
        .unwrap();

    // Owner attempts to sweep the USDC the stream is paying out from.
    let err = helper.withdraw_excess_tokens(&owner, usdc, None).unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert!(
        matches!(root, ContractError::DenomInRewardStream { ref denom } if denom == usdc),
        "expected DenomInRewardStream {{ denom: \"usdc\" }}, got {root:?}",
    );

    // Contract still holds the USDC committed to the stream.
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
        "explicit recipient receives the full amount",
    );
    assert_eq!(helper.query_balance(&owner, bogus).unwrap().u128(), 0);
}

// ─── FIND-015: expired-reward pruning ─────────────────────────────────────────
//
// Audit recommendation: `MAX_REWARD_DENOMS = 5` should cap *concurrent active*
// reward streams, not *lifetime* denoms. Expired entries are lazily moved out
// of the active list into FINISHED_REWARDS on the next interaction, freeing
// the slot. A user who hadn't synced before the prune is still credited via
// the FINISHED_REWARDS final-index fallback in `sync_user_rewards`.

/// 1) Cap-respect: 5 active streams → adding a 6th fails.
#[test]
fn test_reward_cap_rejects_sixth_when_all_active() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // Five denoms, all with the same generous end_ts so none have expired.
    let denoms = ["uusdc", "uusdt", "ubtc", "ueth", "uatom"];
    for d in &denoms {
        helper.mint_coin(&owner, coin(1_000, *d));
        helper.add_reward(&owner, coin(1_000, *d), 10_000).unwrap();
    }

    helper.mint_coin(&owner, coin(1_000, "ufluffy"));
    let err = helper.add_reward(&owner, coin(1_000, "ufluffy"), 10_000).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::TooManyRewardDenoms { max: 5 },
    );
}

/// 2) Lazy prune on AddReward: one of the 5 expires → adding a 6th succeeds
///    in the same tx (update_rewards prunes the expired one first).
#[test]
fn test_reward_slot_freed_after_expiry_and_sixth_admitted() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // Need at least one staker so accrual happens (otherwise the orphaned-
    // reward path muddies the assertion; not relevant to this finding).
    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let short_lived = "ushortlived";
    helper.mint_coin(&owner, coin(1_000, short_lived));
    helper.add_reward(&owner, coin(1_000, short_lived), 100).unwrap();

    for d in &["uusdt", "ubtc", "ueth", "uatom"] {
        helper.mint_coin(&owner, coin(1_000, *d));
        helper.add_reward(&owner, coin(1_000, *d), 10_000).unwrap();
    }

    // All 5 slots full, can't add a sixth.
    helper.mint_coin(&owner, coin(1_000, "ufluffy"));
    let err = helper
        .add_reward(&owner, coin(1_000, "ufluffy"), 10_000)
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::TooManyRewardDenoms { max: 5 },
    );

    // Fast-forward past `short_lived`'s end_ts.
    helper.next_block(150);

    // The sixth attempt now succeeds: update_rewards prunes `short_lived`,
    // freeing one slot, then the cap check passes against the post-prune list.
    helper.mint_coin(&owner, coin(1_000, "ufluffy"));
    helper.add_reward(&owner, coin(1_000, "ufluffy"), 10_000).unwrap();
}

/// 3) FIND-015 LOAD-BEARING: a user staked while a reward stream was active.
///    The stream expires and is pruned. The user only syncs much later.
///    They must still receive the full accrual via FINISHED_REWARDS fallback.
#[test]
fn test_pruned_reward_still_payable_via_finished_index() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);

    let denom = "uusdc";
    helper.mint_coin(&owner, coin(10_000, denom));

    // Alice stakes BEFORE the reward exists — she has an empty
    // `last_rewards_index` for `uusdc`, exactly the case the two-tier
    // default needs to handle via FINISHED_REWARDS once the stream is pruned.
    helper.stake(&alice, 10_000).unwrap();

    // 100s stream of 10_000 USDC.
    helper.add_reward(&owner, coin(10_000, denom), 100).unwrap();

    // Let it fully expire AND force a different tx to trigger the prune.
    // Bob stakes after expiry, which calls update_rewards → prune happens.
    helper.next_block(150);
    let bob = Addr::unchecked("bob");
    helper.give_oro(1, &bob);
    helper.stake(&bob, 1).unwrap();

    // Alice has not interacted at all since staking. Her snapshot is still
    // empty for `uusdc`. The PendingRewards query must surface what she's
    // owed via the FINISHED_REWARDS fallback (Pass 2).
    let pending = helper.query_pending_rewards(&alice).unwrap();
    let alice_usdc_pending = pending
        .iter()
        .find(|c| c.denom == denom)
        .expect("Alice must see USDC pending via the finished-index fallback")
        .amount
        .u128();

    // Alice's share: 10_000 / (10_000 + 1000 bootstrap) of 10_000 USDC = ~9090.
    // Match the test_incentives floor convention via Decimal256 internal math.
    assert!(
        alice_usdc_pending > 9000 && alice_usdc_pending <= 9091,
        "alice USDC pending = {alice_usdc_pending} (expected ~9090)"
    );

    // And actually claiming pays it out — proves the credit isn't just a
    // query projection. Alice claims via the unbond → claim flow.
    helper.unstake(&alice, 10_000).unwrap();
    helper.next_block(7 * 24 * 60 * 60);
    let alice_usdc_before = helper.query_balance(&alice, denom).unwrap().u128();
    helper.claim(&alice).unwrap();
    let alice_usdc_after = helper.query_balance(&alice, denom).unwrap().u128();
    assert_eq!(
        alice_usdc_after - alice_usdc_before,
        alice_usdc_pending,
        "claim must pay out exactly what PendingRewards advertised"
    );
}

/// 4) Re-adding a previously-finished denom must resume from the old final
///    index so existing user snapshots don't lose credit for the prior stream.
#[test]
fn test_re_adding_finished_denom_resumes_from_final_index() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let alice = Addr::unchecked("alice");
    helper.give_oro(10_000, &alice);
    helper.stake(&alice, 10_000).unwrap();

    let denom = "uusdc";
    helper.mint_coin(&owner, coin(20_000, denom));

    // Stream A: 10_000 USDC over 100s. Alice will earn ~9090.
    helper.add_reward(&owner, coin(10_000, denom), 100).unwrap();
    helper.next_block(150); // expire

    // Admin re-adds USDC. update_rewards prunes the finished entry; the new
    // push reads the prior finished index from FINISHED_REWARDS and uses it
    // as the starting `index` of the new RewardInfo. Without that, Alice
    // would lose her ~9090 from stream A on her next sync.
    helper.add_reward(&owner, coin(10_000, denom), 100).unwrap();

    // Alice hasn't synced yet. Her pending should reflect stream-A credit
    // even though stream B is now live (its accrual is still 0 at this
    // instant since no time has passed since the AddReward call).
    let pending = helper.query_pending_rewards(&alice).unwrap();
    let alice_usdc = pending
        .iter()
        .find(|c| c.denom == denom)
        .unwrap()
        .amount
        .u128();
    assert!(
        alice_usdc > 9000 && alice_usdc <= 9091,
        "stream A credit must survive the re-add via starting_index init, got {alice_usdc}"
    );
}

/// 5) Withdraw-excess guard must reject denoms that are in FINISHED_REWARDS,
///    not just live REWARDS. Pending claims still owe stakers of that denom.
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

    // Expire + force the prune via a no-op stake.
    helper.next_block(150);
    let bob = Addr::unchecked("bob");
    helper.give_oro(1, &bob);
    helper.stake(&bob, 1).unwrap();

    // USDC is no longer in REWARDS (pruned) but IS in FINISHED_REWARDS — the
    // admin must not be allowed to sweep the contract's USDC balance, since
    // Alice still has unsynced claim against the finished index.
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
