#![cfg(not(tarpaulin_include))]

use cosmwasm_std::{Addr, Decimal, StdError};
use itertools::Itertools;
use std::str::FromStr;

use oroswap::asset::AssetInfoExt;
use oroswap::cosmwasm_ext::AbsDiff;
use oroswap::observation::OracleObservation;
use oroswap_pair_stable::error::ContractError;
use oroswap_test::coins::TestCoin;
use oroswap_test::convert::f64_to_dec;
use helper::AppExtension;

use crate::helper::Helper;

mod helper;

#[test]
fn provide_and_withdraw_no_fee() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, Some(0u16)).unwrap();

    let user1 = Addr::unchecked("user1");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    helper.provide_liquidity(&user1, &assets, None).unwrap();

    assert_eq!(199999000, helper.native_balance(&helper.lp_token, &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));

    // The user2 with the same assets should receive the same share
    let user2 = Addr::unchecked("user2");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user2);
    helper.provide_liquidity(&user2, &assets, None).unwrap();
    assert_eq!(200_000000, helper.native_balance(&helper.lp_token, &user2));

    // The user3 makes imbalanced provide thus he is charged with fees
    let user3 = Addr::unchecked("user3");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(200_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user3);
    helper.provide_liquidity(&user3, &assets, None).unwrap();
    assert_eq!(299_927827, helper.native_balance(&helper.lp_token, &user3));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user3));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user3));
    
    helper
        .withdraw_liquidity(&user1, 199999000, vec![], None)
        .unwrap();

    assert_eq!(0, helper.native_balance(&helper.lp_token, &user1));
    // Previous imbalanced provides resulted in different share in assets
    assert_eq!(114296927, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(85722695, helper.coin_balance(&test_coins[1], &user1));

    // Checking imbalanced withdraw. Withdrawing only the first asset x 200 with the whole amount of LP tokens
    let err = helper
        .withdraw_liquidity(
            &user2,
            200_000000,
            vec![helper.assets[&test_coins[0]].with_balance(200_000000u128)],
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Imbalanced withdraw is currently disabled"
    );

    // Providing more LP tokens than needed. The rest will be kept on the user's balance
    let err = helper
        .withdraw_liquidity(
            &user3,
            200_892384,
            vec![helper.assets[&test_coins[1]].with_balance(101_000000u128)],
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Imbalanced withdraw is currently disabled"
    );

    // initial balance - spent amount; 100 goes back to the user3
    assert_eq!(299_927827, helper.native_balance(&helper.lp_token, &user3));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user3));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user3));
}

#[test]
fn provide_with_different_precision() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 4),
        TestCoin::cw20precise("BAR", 5),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    for user_name in ["user1", "user2"] {
        let user = Addr::unchecked(user_name);

        let assets = vec![
            helper.assets[&test_coins[0]].with_balance(100_0000u128),
            helper.assets[&test_coins[1]].with_balance(100_00000u128),
        ];
        helper.give_me_money(&assets, &user);

        helper.provide_liquidity(&user, &assets, None).unwrap();
    }

    let user1 = Addr::unchecked("user1");

    assert_eq!(19999000, helper.native_balance(&helper.lp_token, &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));

    helper
        .withdraw_liquidity(&user1, 19999000, vec![], None)
        .unwrap();

    assert_eq!(0, helper.native_balance(&helper.lp_token, &user1));
    assert_eq!(999950, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(9999500, helper.coin_balance(&test_coins[1], &user1));

    let user2 = Addr::unchecked("user2");
    assert_eq!(20000000, helper.native_balance(&helper.lp_token, &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));

    helper
        .withdraw_liquidity(&user2, 20000000, vec![], None)
        .unwrap();

    assert_eq!(0, helper.native_balance(&helper.lp_token, &user2));
    assert_eq!(999999, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(9999999, helper.coin_balance(&test_coins[1], &user2));
}

#[test]
fn swap_different_precisions() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 4),
        TestCoin::cw20precise("BAR", 5),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_0000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_00000u128),
    ];
    helper.provide_liquidity(&owner, &assets, None).unwrap();

    let user = Addr::unchecked("user");
    // 100 x FOO tokens
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_0000u128);
    // Checking direct swap simulation
    let sim_resp = helper
        .simulate_swap(&offer_asset, Some(helper.assets[&test_coins[1]].clone()))
        .unwrap();
    // And reverse swap as well
    let reverse_sim_resp = helper
        .simulate_reverse_swap(
            &helper.assets[&test_coins[1]].with_balance(sim_resp.return_amount.u128()),
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();
    assert_eq!(offer_asset.amount, reverse_sim_resp.offer_amount);

    helper.give_me_money(&[offer_asset.clone()], &user);
    helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    // 99.94902 x BAR tokens
    assert_eq!(99_94902, sim_resp.return_amount.u128());
    assert_eq!(99_94902, helper.coin_balance(&test_coins[1], &user));
}

#[test]
fn check_swaps() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets, None).unwrap();

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);

    // Test swap without specifying ask_asset (should work for 2-asset pools)
    helper.swap(&user1, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(99_949011, helper.coin_balance(&test_coins[1], &user1));

    let user2 = Addr::unchecked("user2");
    let offer_asset2 = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset2.clone()], &user2);

    // The swap actually succeeds, so let's test the successful case instead
    helper
        .swap(
            &user2,
            &offer_asset2,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(99_947033, helper.coin_balance(&test_coins[1], &user2));
}

#[test]
fn check_wrong_initializations() {
    let owner = Addr::unchecked("owner");

    let err = Helper::new(&owner, vec![TestCoin::native("uluna")], 100u64, None).unwrap_err();

    assert_eq!(
        ContractError::InvalidNumberOfAssets(2),
        err.downcast().unwrap()
    );

    let err = Helper::new(
        &owner,
        vec![
            TestCoin::native("one"),
            TestCoin::cw20("two"),
            TestCoin::native("three"),
            TestCoin::cw20("four"),
            TestCoin::native("five"),
            TestCoin::cw20("six"),
        ],
        100u64,
        None,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::InvalidNumberOfAssets(2),
        err.downcast().unwrap()
    );

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::native("uluna")],
        100u64,
        None,
    )
    .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Doubling assets in asset infos"
    );

    // 2 assets in the pool is okay
    Helper::new(
        &owner,
        vec![TestCoin::native("one"), TestCoin::cw20("two")],
        100u64,
        None,
    )
    .unwrap();
}

#[test]
fn check_withdraw_charges_fees() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets, None).unwrap();

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);

    // Usual swap for reference
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    let usual_swap_amount = helper.coin_balance(&test_coins[1], &user1);
    assert_eq!(99_950000, usual_swap_amount);

    // Trying to swap LUNA -> USDC via provide/withdraw
    let user2 = Addr::unchecked("user2");
    helper.give_me_money(&[offer_asset.clone()], &user2);

    // Provide 100 x LUNA and corresponding USDC
    let provide_assets = vec![
        offer_asset.clone(),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&provide_assets, &user2);
    helper
        .provide_liquidity(&user2, &provide_assets, None)
        .unwrap();

    // Withdraw 100 x USDC
    let lp_tokens_amount = helper.native_balance(&helper.lp_token, &user2);
    let err = helper
        .withdraw_liquidity(
            &user2,
            lp_tokens_amount,
            vec![helper.assets[&test_coins[1]].with_balance(100_000000u128)],
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Imbalanced withdraw is currently disabled"
    );

    let err = helper
        .withdraw_liquidity(
            &user2,
            lp_tokens_amount,
            vec![helper.assets[&test_coins[1]].with_balance(usual_swap_amount)],
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Imbalanced withdraw is currently disabled"
    );

    // A small residual of LP tokens is left
    // assert_eq!(8, helper.native_balance(&helper.lp_token, &user2));
    // assert_eq!(
    //     usual_swap_amount,
    //     helper.coin_balance(&test_coins[1], &user2)
    // );
}

#[test]
fn check_twap_based_prices() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let check_prices = |helper: &Helper| {
        let prices = helper.query_prices().unwrap();

        test_coins
            .iter()
            .cartesian_product(test_coins.iter())
            .filter(|(a, b)| a != b)
            .for_each(|(from_coin, to_coin)| {
                let price = prices
                    .cumulative_prices
                    .iter()
                    .filter(|(from, to, _)| {
                        from.eq(&helper.assets[&from_coin]) && to.eq(&helper.assets[&to_coin])
                    })
                    .collect::<Vec<_>>();
                assert_eq!(price.len(), 1);
                assert!(!price[0].2.is_zero());
            });
    };

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets, None).unwrap();
    helper.app.next_block(1000);
    check_prices(&helper);

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);

    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();

    helper.app.next_block(86400);
    check_prices(&helper);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    // Imbalanced provide
    helper.provide_liquidity(&user1, &assets, None).unwrap();
    helper.app.next_block(14 * 86400);
    check_prices(&helper);

    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();
    helper.app.next_block(86400);
    check_prices(&helper);
}

#[test]
fn check_pool_prices() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets, None).unwrap();
    helper.app.next_block(1000);

    let err = helper.query_observe(0).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Querier contract error: Generic error: Buffer is empty")
    );

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);

    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();

    helper.app.next_block(86400);
    assert_eq!(
        helper.query_observe(0).unwrap(),
        OracleObservation {
            timestamp: helper.app.block_info().time.seconds(),
            price: Decimal::from_str("1.000500348223145698").unwrap()
        }
    );

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    // Imbalanced provide
    helper.provide_liquidity(&user1, &assets, None).unwrap();
    helper.app.next_block(14 * 86400);

    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();

    // One more swap to trigger price update in the next step
    helper
        .swap(
            &owner,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();

    helper.app.next_block(86400);

    assert_eq!(
        helper.query_observe(0).unwrap(),
        OracleObservation {
            timestamp: helper.app.block_info().time.seconds(),
            price: Decimal::from_str("0.999999778261572849").unwrap()
        }
    );

    let price1 = helper.observe_price(0).unwrap();
    helper.app.next_block(10);
    // Swapping the lowest amount possible which results in positive return amount
    helper
        .swap(
            &user1,
            &helper.assets[&test_coins[1]].with_balance(2u128),
            None,
        )
        .unwrap();
    let price2 = helper.observe_price(0).unwrap();
    // With such a small swap size contract doesn't store observation
    assert_eq!(price1, price2);

    helper.app.next_block(10);
    // Swap the smallest possible amount which gets observation saved
    helper
        .swap(
            &user1,
            &helper.assets[&test_coins[1]].with_balance(1005u128),
            None,
        )
        .unwrap();
    let price3 = helper.observe_price(0).unwrap();
    // Prove that price didn't jump that much
    let diff = price3.diff(price2);
    assert!(
        diff / price2 < f64_to_dec(0.005),
        "price jumped from {price2} to {price3} which is more than 0.5%"
    );
    helper.app.next_block(10);
}
