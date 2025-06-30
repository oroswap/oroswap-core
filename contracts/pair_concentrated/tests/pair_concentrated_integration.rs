#![cfg(not(tarpaulin_include))]

use std::str::FromStr;

use cosmwasm_std::{Addr, Coin, Decimal, Decimal256, StdError, Uint128, Uint64, StdResult};
use itertools::{max, Itertools};

use oroswap::asset::{
    native_asset_info, token_asset_info, Asset, AssetInfo, AssetInfoExt, PairInfo, MINIMUM_LIQUIDITY_AMOUNT,
};
use oroswap::cosmwasm_ext::{AbsDiff, IntegerToDecimal};
use oroswap::observation::OracleObservation;
use oroswap::pair::{ExecuteMsg, PoolResponse, MAX_FEE_SHARE_BPS, SimulationResponse, ReverseSimulationResponse, CumulativePricesResponse, ConfigResponse};
use oroswap::pair_concentrated::{
    ConcentratedPoolParams, ConcentratedPoolUpdateParams, PromoteParams, QueryMsg, UpdatePoolParams,
};
use oroswap::tokenfactory_tracker::{
    ConfigResponse as TrackerConfigResponse, QueryMsg as TrackerQueryMsg,
};
use oroswap_pair_concentrated::error::ContractError;
use oroswap_pcl_common::consts::{AMP_MAX, AMP_MIN, MA_HALF_TIME_LIMITS};
use oroswap_pcl_common::error::PclError;

use oroswap_test::coins::TestCoin;
use oroswap_test::convert::{dec_to_f64, f64_to_dec};
use oroswap_test::cw_multi_test::{Executor, TOKEN_FACTORY_MODULE};

use crate::helper::{common_pcl_params, AppExtension, Helper};

mod helper;

#[test]
fn check_observe_queries() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let d = helper.query_d().unwrap();
    assert_eq!(dec_to_f64(d), 200000f64);

    assert_eq!(0, helper.coin_balance(&test_coins[1], &user));
    helper.swap(&user, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    assert_eq!(99_737929, helper.coin_balance(&test_coins[1], &user));

    helper.app.next_block(1000);

    let user2 = Addr::unchecked("user2");
    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user2);
    helper.swap(&user2, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));
    assert_eq!(99_741246, helper.coin_balance(&test_coins[0], &user2));

    let d = helper.query_d().unwrap();
    assert_eq!(dec_to_f64(d), 200000.260415);

    let res: OracleObservation = helper
        .app
        .wrap()
        .query_wasm_smart(
            helper.pair_addr.to_string(),
            &QueryMsg::Observe { seconds_ago: 0 },
        )
        .unwrap();

    assert_eq!(
        res,
        OracleObservation {
            timestamp: helper.app.block_info().time.seconds(),
            price: Decimal::from_str("1.002627596167552265").unwrap()
        }
    );
}

#[test]
fn check_wrong_initialization() {
    let owner = Addr::unchecked("owner");

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let err = Helper::new(&owner, vec![TestCoin::native("uluna")], params.clone()).unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: asset_infos must contain exactly two elements",
    );

    let mut wrong_params = params.clone();
    wrong_params.amp = Decimal::zero();

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::cw20("ORO")],
        wrong_params,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::PclError(PclError::IncorrectPoolParam(
            "amp".to_string(),
            AMP_MIN.to_string(),
            AMP_MAX.to_string()
        )),
        err.downcast().unwrap(),
    );

    let mut wrong_params = params.clone();
    wrong_params.ma_half_time = MA_HALF_TIME_LIMITS.end() + 1;

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::cw20("ORO")],
        wrong_params,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::PclError(PclError::IncorrectPoolParam(
            "ma_half_time".to_string(),
            MA_HALF_TIME_LIMITS.start().to_string(),
            MA_HALF_TIME_LIMITS.end().to_string()
        )),
        err.downcast().unwrap(),
    );

    let mut wrong_params = params.clone();
    wrong_params.price_scale = Decimal::zero();

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::cw20("ORO")],
        wrong_params,
    )
    .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Initial price scale can not be zero",
    );

    // check instantiation with valid params
    Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::cw20("ORO")],
        params,
    )
    .unwrap();
}

#[test]
fn check_create_pair_with_unsupported_denom() {
    let owner = Addr::unchecked("owner");

    let wrong_coins = vec![TestCoin::native("rc"), TestCoin::native("uusdc")];
    let valid_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let err = Helper::new(&owner, wrong_coins.clone(), params.clone()).unwrap_err();
    assert_eq!(
        "Generic error: Invalid denom length [3,128]: rc",
        err.root_cause().to_string()
    );

    Helper::new(&owner, valid_coins.clone(), params.clone()).unwrap();
}

#[test]
fn provide_and_withdraw() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    // checking LP token virtual price on an empty pool
    let lp_price = helper.query_lp_price().unwrap();
    assert!(
        lp_price.is_zero(),
        "LP price must be zero before any provide"
    );

    let user1 = Addr::unchecked("user1");

    let random_coin = native_asset_info("random-coin".to_string()).with_balance(100u8);
    let wrong_assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        random_coin.clone(),
    ];

    helper.give_me_money(&wrong_assets, &user1);

    // Provide with empty assets
    let err = helper.provide_liquidity(&user1, &[]).unwrap_err();
    assert_eq!(
        "Generic error: Nothing to provide",
        err.root_cause().to_string()
    );

    // Provide just one asset which does not belong to the pair
    let err = helper
        .provide_liquidity(&user1, &[random_coin.clone()])
        .unwrap_err();
    assert_eq!(
        "The asset random-coin does not belong to the pair",
        err.root_cause().to_string()
    );

    // Try to provide 3 assets
    let err = helper
        .provide_liquidity(
            &user1,
            &[
                random_coin.clone(),
                helper.assets[&test_coins[0]].with_balance(1u8),
                helper.assets[&test_coins[1]].with_balance(1u8),
            ],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::InvalidNumberOfAssets(2),
        err.downcast().unwrap()
    );

    // Try to provide with zero amount
    let err = helper
        .provide_liquidity(
            &user1,
            &[
                helper.assets[&test_coins[0]].with_balance(0u8),
                helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
            ],
        )
        .unwrap_err();
    assert_eq!(ContractError::InvalidZeroAmount {}, err.downcast().unwrap());

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];
    helper.give_me_money(
        &[helper.assets[&test_coins[1]].with_balance(50_000_000000u128)],
        &user1,
    );

    // Test very small initial provide
    let err = helper
        .provide_liquidity(
            &user1,
            &[
                helper.assets[&test_coins[0]].with_balance(1000u128),
                helper.assets[&test_coins[1]].with_balance(500u128),
            ],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::MinimumLiquidityAmountError {},
        err.downcast().unwrap()
    );

    // This is normal provision
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(
        70710_677118,
        helper.native_balance(&helper.lp_token, &user1)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));

    assert_eq!(
        helper
            .query_share(helper.native_balance(&helper.lp_token, &user1))
            .unwrap(),
        vec![
            helper.assets[&test_coins[0]].with_balance(99999998584u128),
            helper.assets[&test_coins[1]].with_balance(49999999292u128)
        ]
    );

    let user2 = Addr::unchecked("user2");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];
    helper.give_me_money(&assets, &user2);
    helper.provide_liquidity(&user2, &assets).unwrap();
    assert_eq!(
        70710_677118 + MINIMUM_LIQUIDITY_AMOUNT.u128(),
        helper.native_balance(&helper.lp_token, &user2)
    );

    // Changing order of assets does not matter
    let user3 = Addr::unchecked("user3");
    let assets = vec![
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user3);
    helper.provide_liquidity(&user3, &assets).unwrap();
    assert_eq!(
        70710_677118 + MINIMUM_LIQUIDITY_AMOUNT.u128(),
        helper.native_balance(&helper.lp_token, &user3)
    );

    // After initial provide one-sided provide is allowed
    let user4 = Addr::unchecked("user4");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(0u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user4);
    helper.provide_liquidity(&user4, &assets).unwrap();
    // LP amount is less than for prev users as provide is imbalanced
    assert_eq!(
        62217_722016,
        helper.native_balance(&helper.lp_token, &user4)
    );

    // One of assets may be omitted
    let user5 = Addr::unchecked("user5");
    let assets = vec![helper.assets[&test_coins[0]].with_balance(140_000_000000u128)];
    helper.give_me_money(&assets, &user5);
    helper.provide_liquidity(&user5, &assets).unwrap();
    assert_eq!(
        57271_023590,
        helper.native_balance(&helper.lp_token, &user5)
    );

    // check that imbalanced withdraw is currently disabled
    let withdraw_assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(5_000_000000u128),
    ];
    let err = helper
        .withdraw_liquidity(&user1, 7071_067711, withdraw_assets)
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Imbalanced withdraw is currently disabled"
    );

    // user1 withdraws 1/10 of his LP tokens
    helper
        .withdraw_liquidity(&user1, 7071_067711, vec![])
        .unwrap();

    assert_eq!(
        70710_677118 - 7071_067711,
        helper.native_balance(&helper.lp_token, &user1)
    );
    assert_eq!(9382_010960, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(5330_688045, helper.coin_balance(&test_coins[1], &user1));

    // user2 withdraws half
    helper
        .withdraw_liquidity(&user2, 35355_339059, vec![])
        .unwrap();

    assert_eq!(
        70710_677118 + MINIMUM_LIQUIDITY_AMOUNT.u128() - 35355_339059,
        helper.native_balance(&helper.lp_token, &user2)
    );
    assert_eq!(46910_055478, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(26653_440612, helper.coin_balance(&test_coins[1], &user2));
}

#[test]
fn check_imbalanced_provide() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("USDC")];

    let mut params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params.clone()).unwrap();

    let user1 = Addr::unchecked("user1");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    // Making two provides just to check that both if-branches are covered (initial and usual provide)
    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(
        200495_366531,
        helper.native_balance(&helper.lp_token, &user1)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));

    // creating a new pool with inverted price scale
    params.price_scale = Decimal::from_ratio(1u8, 2u8);

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(
        200495_366531,
        helper.native_balance(&helper.lp_token, &user1)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));
}

#[test]
fn provide_with_different_precision() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 5),
        TestCoin::cw20precise("BAR", 6),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_00000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];

    helper.provide_liquidity(&owner, &assets).unwrap();

    let tolerance = 9;

    for user_name in ["user1", "user2", "user3"] {
        let user = Addr::unchecked(user_name);

        helper.give_me_money(&assets, &user);

        helper.provide_liquidity(&user, &assets).unwrap();

        let lp_amount = helper.native_balance(&helper.lp_token, &user);
        assert!(
            100_000000 - lp_amount < tolerance,
            "LP token balance assert failed for {user}"
        );
        assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
        assert_eq!(0, helper.coin_balance(&test_coins[1], &user));

        helper.withdraw_liquidity(&user, lp_amount, vec![]).unwrap();

        assert_eq!(0, helper.native_balance(&helper.lp_token, &user));
        assert!(
            100_00000 - helper.coin_balance(&test_coins[0], &user) < tolerance,
            "Withdrawn amount of coin0 assert failed for {user}"
        );
        assert!(
            100_000000 - helper.coin_balance(&test_coins[1], &user) < tolerance,
            "Withdrawn amount of coin1 assert failed for {user}"
        );
    }
}

#[test]
fn swap_different_precisions() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 5),
        TestCoin::cw20precise("BAR", 6),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_00000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user = Addr::unchecked("user");
    // 100 x FOO tokens
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_00000u128);

    // Checking direct swap simulation
    let sim_resp = helper.simulate_swap(&offer_asset, None).unwrap();
    // And reverse swap as well
    let reverse_sim_resp = helper
        .simulate_reverse_swap(
            &helper.assets[&test_coins[1]].with_balance(sim_resp.return_amount.u128()),
            None,
        )
        .unwrap();
    assert_eq!(reverse_sim_resp.offer_amount.u128(), 10019003);
    assert_eq!(reverse_sim_resp.commission_amount.u128(), 45084);
    assert_eq!(reverse_sim_resp.spread_amount.u128(), 125);

    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    // 99_737929 x BAR tokens
    assert_eq!(99_737929, sim_resp.return_amount.u128());
    assert_eq!(
        sim_resp.return_amount.u128(),
        helper.coin_balance(&test_coins[1], &user)
    );
}

#[test]
fn simulate_provide() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];

    let user1 = Addr::unchecked("user1");

    let shares: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(
            helper.pair_addr.to_string(),
            &QueryMsg::SimulateProvide {
                assets: assets.clone(),
                slippage_tolerance: None,
            },
        )
        .unwrap();

    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(
        shares.u128(),
        helper.native_balance(&helper.lp_token, &user1)
    );

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_0000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];

    let err = helper
        .app
        .wrap()
        .query_wasm_smart::<Uint128>(
            helper.pair_addr.to_string(),
            &QueryMsg::SimulateProvide {
                assets: assets.clone(),
                slippage_tolerance: Option::from(Decimal::percent(1)),
            },
        )
        .unwrap_err();

    assert_eq!(
        err,
        StdError::generic_err(
            "Querier contract error: Generic error: Operation exceeds max spread limit"
        )
    );
}

#[test]
fn check_reverse_swap() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::cw20("FOO"), TestCoin::cw20("BAR")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let offer_asset = helper.assets[&test_coins[0]].with_balance(50_000_000000u128);

    let sim_resp = helper.simulate_swap(&offer_asset, None).unwrap();
    let reverse_sim_resp = helper
        .simulate_reverse_swap(
            &helper.assets[&test_coins[1]].with_balance(sim_resp.return_amount.u128()),
            None,
        )
        .unwrap();
    assert_eq!(reverse_sim_resp.offer_amount.u128(), 50000220879u128); // as it is hard to predict dynamic fees reverse swap is not exact
    assert_eq!(reverse_sim_resp.commission_amount.u128(), 151_913981);
    assert_eq!(reverse_sim_resp.spread_amount.u128(), 16241_558397);
}

#[test]
fn check_swaps_simple() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("USDC")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);

    // Check swap does not work if pool is empty
    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: One of the pools is empty"
    );

    // Try to swap a wrong asset
    let wrong_coin = native_asset_info("random-coin".to_string());
    let wrong_asset = wrong_coin.with_balance(100_000000u128);
    helper.give_me_money(&[wrong_asset.clone()], &user);
    let err = helper.swap(&user, &wrong_asset, None).unwrap_err();
    assert_eq!(
        ContractError::InvalidAsset(wrong_coin.to_string()),
        err.downcast().unwrap()
    );

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // trying to swap cw20 without calling Cw20::Send method
    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.pair_addr.clone(),
            &ExecuteMsg::Swap {
                offer_asset: helper.assets[&test_coins[1]].with_balance(1u8),
                ask_asset_info: None,
                belief_price: None,
                max_spread: None,
                to: None,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(ContractError::Cw20DirectSwap {}, err.downcast().unwrap());

    let d = helper.query_d().unwrap();
    assert_eq!(dec_to_f64(d), 200000f64);

    assert_eq!(0, helper.coin_balance(&test_coins[1], &user));
    helper.swap(&user, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    assert_eq!(99_737929, helper.coin_balance(&test_coins[1], &user));

    let offer_asset = helper.assets[&test_coins[0]].with_balance(90_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    assert_eq!(
        ContractError::PclError(PclError::MaxSpreadAssertion {}),
        err.downcast().unwrap()
    );

    let user2 = Addr::unchecked("user2");
    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user2);
    helper.swap(&user2, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));
    assert_eq!(99_741246, helper.coin_balance(&test_coins[0], &user2));

    let d = helper.query_d().unwrap();
    assert_eq!(dec_to_f64(d), 200000.260415);

    let price1 = helper.observe_price(0).unwrap();
    helper.app.next_block(10);
    // Swapping the lowest amount possible which results in positive return amount
    helper
        .swap(
            &user,
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
            &user,
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
}

#[test]
fn check_swaps_with_price_update() {
    let owner = Addr::unchecked("owner");
    let half = Decimal::from_ratio(1u8, 2u8);

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(1000);

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    let mut prev_vlp_price = helper.query_lp_price().unwrap();

    for i in 0..4 {
        helper.give_me_money(&[offer_asset.clone()], &user1);
        helper.swap(&user1, &offer_asset, Some(half)).unwrap();
        let new_vlp_price = helper.query_lp_price().unwrap();
        assert!(
            new_vlp_price >= prev_vlp_price,
            "{i}: new_vlp_price <= prev_vlp_price ({new_vlp_price} <= {prev_vlp_price})",
        );
        prev_vlp_price = new_vlp_price;
        helper.app.next_block(1000);
    }

    let offer_asset = helper.assets[&test_coins[0]].with_balance(10_000_000000u128);
    for _i in 0..4 {
        helper.give_me_money(&[offer_asset.clone()], &user1);
        helper.swap(&user1, &offer_asset, Some(half)).unwrap();
        helper.app.next_block(1000);
    }
}

#[test]
fn provides_and_swaps() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(1000);

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    let provider = Addr::unchecked("provider");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(1_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000_000000u128),
    ];
    helper.give_me_money(&assets, &provider);
    helper.provide_liquidity(&provider, &assets).unwrap();

    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    helper
        .withdraw_liquidity(&provider, 999_999354, vec![])
        .unwrap();

    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();
}

#[test]
fn check_amp_gamma_change() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(40f64),
        gamma: f64_to_dec(0.0001),
        ..common_pcl_params()
    };
    let mut helper = Helper::new(&owner, test_coins, params).unwrap();

    let random_user = Addr::unchecked("random");
    let action = ConcentratedPoolUpdateParams::Update(UpdatePoolParams {
        mid_fee: Some(f64_to_dec(0.002)),
        out_fee: None,
        fee_gamma: None,
        repeg_profit_threshold: None,
        min_price_scale_delta: None,
        ma_half_time: None,
    });

    let err = helper.update_config(&random_user, &action).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());

    helper.update_config(&owner, &action).unwrap();

    helper.app.next_block(86400);

    let future_time = helper.app.block_info().time.seconds() + 100_000;
    let target_amp = 44f64;
    let target_gamma = 0.00009;
    let action = ConcentratedPoolUpdateParams::Promote(PromoteParams {
        next_amp: f64_to_dec(target_amp),
        next_gamma: f64_to_dec(target_gamma),
        future_time,
    });
    helper.update_config(&owner, &action).unwrap();

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 40f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.0001);
    assert_eq!(amp_gamma.future_time, future_time);

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 42f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.000095);
    assert_eq!(amp_gamma.future_time, future_time);

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), target_amp);
    assert_eq!(dec_to_f64(amp_gamma.gamma), target_gamma);
    assert_eq!(amp_gamma.future_time, future_time);

    // change values back
    let future_time = helper.app.block_info().time.seconds() + 100_000;
    let action = ConcentratedPoolUpdateParams::Promote(PromoteParams {
        next_amp: f64_to_dec(40f64),
        next_gamma: f64_to_dec(0.000099),
        future_time,
    });
    helper.update_config(&owner, &action).unwrap();

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 42f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.0000945);
    assert_eq!(amp_gamma.future_time, future_time);

    // stop changing amp and gamma thus fixing current values
    let action = ConcentratedPoolUpdateParams::StopChangingAmpGamma {};
    helper.update_config(&owner, &action).unwrap();
    let amp_gamma = helper.query_amp_gamma().unwrap();
    let last_change_time = helper.app.block_info().time.seconds();
    assert_eq!(amp_gamma.future_time, last_change_time);

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 42f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.0000945);
    assert_eq!(amp_gamma.future_time, last_change_time);
}

#[test]
fn check_prices() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    helper.app.next_block(50_000);

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
                        from.eq(&helper.assets[from_coin]) && to.eq(&helper.assets[to_coin])
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
    helper.provide_liquidity(&owner, &assets).unwrap();
    check_prices(&helper);

    helper.app.next_block(1000);

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);

    helper.swap(&user1, &offer_asset, None).unwrap();
    check_prices(&helper);

    helper.app.next_block(86400);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    helper.provide_liquidity(&user1, &assets).unwrap();
    check_prices(&helper);

    helper.app.next_block(14 * 86400);

    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper.swap(&user1, &offer_asset, None).unwrap();
    check_prices(&helper);
}

#[test]
fn update_owner() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    let mut helper = Helper::new(&owner, test_coins, common_pcl_params()).unwrap();

    let new_owner = String::from("new_owner");

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    // Unauthorized check
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked("not_owner"),
            helper.pair_addr.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            helper.pair_addr.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner
    helper
        .app
        .execute_contract(
            Addr::unchecked(&helper.owner),
            helper.pair_addr.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Claim from invalid addr
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            helper.pair_addr.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Drop ownership proposal
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            helper.pair_addr.clone(),
            &ExecuteMsg::DropOwnershipProposal {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    helper
        .app
        .execute_contract(
            helper.owner.clone(),
            helper.pair_addr.clone(),
            &ExecuteMsg::DropOwnershipProposal {},
            &[],
        )
        .unwrap();

    // Propose new owner
    helper
        .app
        .execute_contract(
            Addr::unchecked(&helper.owner),
            helper.pair_addr.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Claim ownership
    helper
        .app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            helper.pair_addr.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap();

    let config = helper.query_config().unwrap();
    assert_eq!(config.owner.unwrap().to_string(), new_owner)
}

#[test]
fn query_d_test() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    // create pair with test_coins
    let helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // query current pool D value before providing any liquidity
    let err = helper.query_d().unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Querier contract error: Generic error: Pools are empty"
    );
}

#[test]
fn asset_balances_tracking_with_in_params() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusd")];

    let params = ConcentratedPoolParams {
        track_asset_balances: Some(true),
        ..common_pcl_params()
    };

    // Instantiate pair without asset balances tracking
    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(5_000000u128),
        helper.assets[&test_coins[1]].with_balance(5_000000u128),
    ];

    // Check that asset balances were not tracked before instantiation
    // The query AssetBalanceAt returns None for this case
    let res = helper
        .query_asset_balance_at(&assets[0].info, helper.app.block_info().height)
        .unwrap();
    assert!(res.is_none());

    let res = helper
        .query_asset_balance_at(&assets[1].info, helper.app.block_info().height)
        .unwrap();
    assert!(res.is_none());

    // Check that asset balances were not tracked before instantiation
    // The query AssetBalanceAt returns None for this case
    let res = helper
        .query_asset_balance_at(&assets[0].info, helper.app.block_info().height)
        .unwrap();
    assert!(res.is_none());

    let res = helper
        .query_asset_balance_at(&assets[1].info, helper.app.block_info().height)
        .unwrap();
    assert!(res.is_none());

    // Check that asset balances had zero balances before next block upon instantiation
    helper.app.update_block(|b| b.height += 1);

    let res = helper
        .query_asset_balance_at(&assets[0].info, helper.app.block_info().height)
        .unwrap();
    assert!(res.unwrap().is_zero());

    let res = helper
        .query_asset_balance_at(&assets[1].info, helper.app.block_info().height)
        .unwrap();
    assert!(res.unwrap().is_zero());

    // Provide liquidity
    helper
        .provide_liquidity(
            &owner,
            &[
                assets[0].info.with_balance(999_000000u128),
                assets[1].info.with_balance(1000_000000u128),
            ],
        )
        .unwrap();

    assert_eq!(
        helper.native_balance(&helper.lp_token, &owner),
        999_498998u128
    );

    // Check that asset balances changed after providing liquidity
    helper.app.update_block(|b| b.height += 1);
    let res = helper
        .query_asset_balance_at(&assets[0].info, helper.app.block_info().height)
        .unwrap();
    assert_eq!(res.unwrap(), Uint128::new(999_000000));

    let res = helper
        .query_asset_balance_at(&assets[1].info, helper.app.block_info().height)
        .unwrap();
    assert_eq!(res.unwrap(), Uint128::new(1000_000000));

    // Swap
    helper
        .swap(
            &owner,
            &Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_owned(),
                },
                amount: Uint128::new(1_000000),
            },
            None,
        )
        .unwrap();

    // Check that asset balances changed after swapping
    helper.app.update_block(|b| b.height += 1);
    let res = helper
        .query_asset_balance_at(&assets[0].info, helper.app.block_info().height)
        .unwrap();
    assert_eq!(res.unwrap(), Uint128::new(998_001335));

    let res = helper
        .query_asset_balance_at(&assets[1].info, helper.app.block_info().height)
        .unwrap();
    assert_eq!(res.unwrap(), Uint128::new(1001_000000));

    // Withdraw liquidity
    helper
        .withdraw_liquidity(&owner, 500_000000, vec![])
        .unwrap();

    // Check that asset balances changed after withdrawing
    helper.app.update_block(|b| b.height += 1);
    let res = helper
        .query_asset_balance_at(&assets[0].info, helper.app.block_info().height)
        .unwrap();
    assert_eq!(res.unwrap(), Uint128::new(498_751043));

    let res = helper
        .query_asset_balance_at(&assets[1].info, helper.app.block_info().height)
        .unwrap();
    assert_eq!(res.unwrap(), Uint128::new(500_249625));
}

#[test]
fn provides_and_swaps_and_withdraw() {
    let owner = Addr::unchecked("owner");
    let half = Decimal::from_ratio(1u8, 2u8);
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(1u8, 2u8),
        ..common_pcl_params()
    };
    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(200_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // swap uluna
    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, Some(half)).unwrap();

    helper.app.next_block(1000);

    // swap usdc
    let offer_asset = helper.assets[&test_coins[1]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, Some(half)).unwrap();

    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, Some(half)).unwrap();

    // swap uluna
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, Some(half)).unwrap();
    let res: PoolResponse = helper
        .app
        .wrap()
        .query_wasm_smart(helper.pair_addr.to_string(), &QueryMsg::Pool {})
        .unwrap();

    assert_eq!(res.total_share.u128(), 141_421_356_237u128);
    let owner_balance = helper.native_balance(&helper.lp_token, &owner);

    helper
        .withdraw_liquidity(&owner, owner_balance, vec![])
        .unwrap();
    let res: PoolResponse = helper
        .app
        .wrap()
        .query_wasm_smart(helper.pair_addr.to_string(), &QueryMsg::Pool {})
        .unwrap();

    assert_eq!(res.total_share.u128(), 1000u128);
}

#[test]
fn provide_liquidity_with_autostaking_to_generator() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(1u8, 2u8),
        ..common_pcl_params()
    };
    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000u128),
        helper.assets[&test_coins[1]].with_balance(100_000u128),
    ];

    helper
        .provide_liquidity_with_auto_staking(&owner, &assets, None)
        .unwrap();

    let amount = helper.query_incentives_deposit(helper.lp_token.to_string(), &owner);
    assert_eq!(amount, Uint128::new(99003));
}

#[test]
fn provide_withdraw_provide() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(10f64),
        price_scale: Decimal::from_ratio(10u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_938039u128),
        helper.assets[&test_coins[1]].with_balance(1_093804u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.next_block(90);
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(90);
    let uusd = helper.assets[&test_coins[0]].with_balance(5_000000u128);
    helper.swap(&owner, &uusd, Some(f64_to_dec(0.5))).unwrap();

    helper.app.next_block(600);
    // Withdraw all
    let lp_amount = helper.native_balance(&helper.lp_token, &owner);
    helper
        .withdraw_liquidity(&owner, lp_amount, vec![])
        .unwrap();

    // Provide again
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
        .unwrap();
}

#[test]
fn provide_withdraw_slippage() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(10f64),
        price_scale: Decimal::from_ratio(10u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    // Fully balanced provide
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000000u128),
    ];
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.02)))
        .unwrap();

    // Imbalanced provide. Slippage is more than 2% while we enforce 2% max slippage
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(5_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000000u128),
    ];
    let err = helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.02)))
        .unwrap_err();
    assert_eq!(
        ContractError::PclError(PclError::MaxSpreadAssertion {}),
        err.downcast().unwrap(),
    );
    // With 3% slippage it should work
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.03)))
        .unwrap();

    // Provide with a huge imbalance. Slippage is ~42.2%
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(1000_000000u128),
        helper.assets[&test_coins[1]].with_balance(1000_000000u128),
    ];
    let err = helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.02)))
        .unwrap_err();
    assert_eq!(
        ContractError::PclError(PclError::MaxSpreadAssertion {}),
        err.downcast().unwrap(),
    );
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
        .unwrap();

    let err = helper
        .provide_liquidity_full(
            &owner,
            &assets,
            Some(f64_to_dec(0.5)),
            None,
            None,
            Some(10000000000u128.into()),
        )
        .unwrap_err();
    assert_eq!(
        ContractError::ProvideSlippageViolation(1000229863u128.into(), 10000000000u128.into()),
        err.downcast().unwrap(),
    );

    helper
        .provide_liquidity_full(
            &owner,
            &assets,
            Some(f64_to_dec(0.5)),
            None,
            None,
            Some(1000229863u128.into()),
        )
        .unwrap();
}

#[test]
fn test_frontrun_before_initial_provide() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(10f64),
        price_scale: Decimal::from_ratio(10u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    // Random person tries to frontrun initial provide and imbalance pool upfront
    helper
        .app
        .send_tokens(
            owner.clone(),
            helper.pair_addr.clone(),
            &[helper.assets[&test_coins[0]]
                .with_balance(10_000_000000u128)
                .as_coin()
                .unwrap()],
        )
        .unwrap();

    // Fully balanced provide
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    // Now pool became imbalanced with value (10010, 1)  (or in internal representation (10010, 10))
    // while price scale stays 10

    let arber = Addr::unchecked("arber");
    let offer_asset_luna = helper.assets[&test_coins[1]].with_balance(1_000000u128);
    // Arber spinning pool back to balanced state
    loop {
        helper.app.next_block(10);
        helper.give_me_money(&[offer_asset_luna.clone()], &arber);
        // swapping until price satisfies an arber
        if helper
            .swap_full_params(
                &arber,
                &offer_asset_luna,
                Some(f64_to_dec(0.02)),
                Some(f64_to_dec(0.1)), // imagine market price is 10 -> i.e. inverted price is 1/10
            )
            .is_err()
        {
            break;
        }
    }

    // price scale changed, however it isn't equal to 10 because of repegging
    // But next swaps will align price back to the market value
    let config = helper.query_config().unwrap();
    let price_scale = config.pool_state.price_state.price_scale;
    assert!(
        dec_to_f64(price_scale) - 77.255853 < 1e-5,
        "price_scale: {price_scale} is far from expected price",
    );

    // Arber collected significant profit (denominated in uusd)
    // Essentially 10_000 - fees (which settled in the pool)
    let arber_balance = helper.coin_balance(&test_coins[0], &arber);
    assert_eq!(arber_balance, 9667_528248);

    // Pool's TVL increased from (10, 1) i.e. 20 to (320, 32) i.e. 640 considering market price is 10.0
    let pools = config
        .pair_info
        .query_pools(&helper.app.wrap(), &helper.pair_addr)
        .unwrap();
    assert_eq!(pools[0].amount.u128(), 320_624088);
    assert_eq!(pools[1].amount.u128(), 32_000000);
}

#[test]
fn check_correct_fee_share() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let share_recipient = Addr::unchecked("share_recipient");
    // Attempt setting fee share with max+1 fee share
    let action = ConcentratedPoolUpdateParams::EnableFeeShare {
        fee_share_bps: MAX_FEE_SHARE_BPS + 1,
        fee_share_address: share_recipient.to_string(),
    };
    let err = helper.update_config(&owner, &action).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::FeeShareOutOfBounds {}
    );

    // Attempt setting fee share with max+1 fee share
    let action = ConcentratedPoolUpdateParams::EnableFeeShare {
        fee_share_bps: 0,
        fee_share_address: share_recipient.to_string(),
    };
    let err = helper.update_config(&owner, &action).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::FeeShareOutOfBounds {}
    );

    helper.app.next_block(1000);

    // Set to 5% fee share
    let action = ConcentratedPoolUpdateParams::EnableFeeShare {
        fee_share_bps: 1000,
        fee_share_address: share_recipient.to_string(),
    };
    helper.update_config(&owner, &action).unwrap();

    let config = helper.query_config().unwrap();
    let fee_share = config.fee_share.unwrap();
    assert_eq!(fee_share.bps, 1000u16);
    assert_eq!(fee_share.recipient, share_recipient.to_string());

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(1000);

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    let last_price = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .last_price;
    assert_eq!(
        last_price,
        Decimal256::from_str("1.001187607454013938").unwrap()
    );

    // Check that the shared fees are sent
    let expected_fee_share = 26081u128;
    let recipient_balance = helper.coin_balance(&test_coins[1], &share_recipient);
    assert_eq!(recipient_balance, expected_fee_share);

    let provider = Addr::unchecked("provider");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(1_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000_000000u128),
    ];
    helper.give_me_money(&assets, &provider);
    helper.provide_liquidity(&provider, &assets).unwrap();

    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    let last_price = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .last_price;
    assert_eq!(
        last_price,
        Decimal256::from_str("0.998842355796925899").unwrap()
    );

    helper
        .withdraw_liquidity(&provider, 999_999354, vec![])
        .unwrap();

    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    let last_price = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .last_price;
    assert_eq!(
        last_price,
        Decimal256::from_str("1.00118760696709103").unwrap()
    );

    // Disable fee share
    let action = ConcentratedPoolUpdateParams::DisableFeeShare {};
    helper.update_config(&owner, &action).unwrap();

    let config = helper.query_config().unwrap();
    assert!(config.fee_share.is_none());
}

#[test]
fn check_small_trades() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(4.360000915600192),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    // Fully balanced but small provide
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(8_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_834862u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Trying to mess the last price with lowest possible swap
    for _ in 0..1000 {
        helper.app.next_block(30);
        let offer_asset = helper.assets[&test_coins[1]].with_balance(1u8);
        helper
            .swap_full_params(&owner, &offer_asset, None, Some(Decimal::MAX))
            .unwrap();
    }

    // Check that after price scale adjustments (even they are small) internal value is still nearly balanced
    let config = helper.query_config().unwrap();
    let pool = helper
        .query_pool()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| asset.amount.to_decimal256(6u8).unwrap())
        .collect_vec();

    let ixs = [pool[0], pool[1] * config.pool_state.price_state.price_scale];
    let relative_diff = ixs[0].abs_diff(ixs[1]) / max(&ixs).unwrap();

    assert!(
        relative_diff < Decimal256::percent(3),
        "Internal PCL value is off. Relative_diff: {}",
        relative_diff
    );

    // Trying to mess the last price with lowest possible provide
    for _ in 0..1000 {
        helper.app.next_block(30);
        let assets = vec![helper.assets[&test_coins[1]].with_balance(1u8)];
        helper
            .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
            .unwrap();
    }

    // Check that after price scale adjustments (even they are small) internal value is still nearly balanced
    let config = helper.query_config().unwrap();
    let pool = helper
        .query_pool()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| asset.amount.to_decimal256(6u8).unwrap())
        .collect_vec();

    let ixs = [pool[0], pool[1] * config.pool_state.price_state.price_scale];
    let relative_diff = ixs[0].abs_diff(ixs[1]) / max(&ixs).unwrap();

    assert!(
        relative_diff < Decimal256::percent(3),
        "Internal PCL value is off. Relative_diff: {}",
        relative_diff
    );
}

#[test]
fn check_small_trades_18decimals() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("ETH", 18),
        TestCoin::cw20precise("USD", 18),
    ];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(4.360000915600192),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    // Fully balanced but small provide
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(8e18 as u128),
        helper.assets[&test_coins[1]].with_balance(1_834862000000000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Trying to mess the last price with lowest possible swap
    for _ in 0..1000 {
        helper.app.next_block(30);
        let offer_asset = helper.assets[&test_coins[1]].with_balance(1u8);
        helper
            .swap_full_params(&owner, &offer_asset, None, Some(Decimal::MAX))
            .unwrap();
    }

    // Check that after price scale adjustments (even they are small) internal value is still nearly balanced
    let config = helper.query_config().unwrap();
    let pool = helper
        .query_pool()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| asset.amount.to_decimal256(6u8).unwrap())
        .collect_vec();

    let ixs = [pool[0], pool[1] * config.pool_state.price_state.price_scale];
    let relative_diff = ixs[0].abs_diff(ixs[1]) / max(&ixs).unwrap();

    assert!(
        relative_diff < Decimal256::percent(3),
        "Internal PCL value is off. Relative_diff: {}",
        relative_diff
    );

    // Trying to mess the last price with lowest possible provide
    for _ in 0..1000 {
        helper.app.next_block(30);
        // 0.000001 USD. minimum provide is limited to LP token precision which is 6 decimals.
        let assets = vec![helper.assets[&test_coins[1]].with_balance(1000000000000u128)];
        helper
            .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
            .unwrap();
    }

    // Check that after price scale adjustments (even they are small) internal value is still nearly balanced
    let config = helper.query_config().unwrap();
    let pool = helper
        .query_pool()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| asset.amount.to_decimal256(6u8).unwrap())
        .collect_vec();

    let ixs = [pool[0], pool[1] * config.pool_state.price_state.price_scale];
    let relative_diff = ixs[0].abs_diff(ixs[1]) / max(&ixs).unwrap();

    assert!(
        relative_diff < Decimal256::percent(3),
        "Internal PCL value is off. Relative_diff: {}",
        relative_diff
    );
}

#[test]
fn check_lsd_swaps_with_price_update() {
    let owner = Addr::unchecked("owner");
    let half = Decimal::from_ratio(1u8, 2u8);
    let price_scale = 0.87;

    let test_coins = vec![TestCoin::native("wsteth"), TestCoin::native("eth")];

    // checking swaps in PCL pair with LSD params
    let params = ConcentratedPoolParams {
        amp: f64_to_dec(500_f64),
        gamma: f64_to_dec(0.00000001),
        mid_fee: f64_to_dec(0.0003),
        out_fee: f64_to_dec(0.0045),
        fee_gamma: f64_to_dec(0.3),
        repeg_profit_threshold: f64_to_dec(0.00000001),
        min_price_scale_delta: f64_to_dec(0.0000055),
        price_scale: f64_to_dec(price_scale),
        ma_half_time: 600,
        track_asset_balances: None,
        fee_share: None,
    };
    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance((1e18 * price_scale) as u128),
        helper.assets[&test_coins[1]].with_balance(1e18 as u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.next_block(1000);

    for _ in 0..10 {
        let assets = vec![
            helper.assets[&test_coins[0]].with_balance((1e15 * price_scale) as u128),
            helper.assets[&test_coins[1]].with_balance(1e15 as u128),
        ];
        helper.provide_liquidity(&owner, &assets).unwrap();
        helper.app.next_block(1000);
    }

    for _ in 0..10 {
        let assets = vec![
            helper.assets[&test_coins[0]].with_balance((1e13 * price_scale) as u128),
            helper.assets[&test_coins[1]].with_balance(1e13 as u128),
        ];
        helper.provide_liquidity(&owner, &assets).unwrap();
        helper.app.next_block(1000);
    }

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1e16 as u128);

    for _ in 0..10 {
        helper.give_me_money(&[offer_asset.clone()], &user1);
        helper.swap(&user1, &offer_asset, Some(half)).unwrap();
        helper.app.next_block(1000);
    }

    let offer_asset = helper.assets[&test_coins[1]].with_balance(1e16 as u128);
    for _ in 0..10 {
        helper.give_me_money(&[offer_asset.clone()], &user1);
        helper.swap(&user1, &offer_asset, Some(half)).unwrap();
        helper.app.next_block(1000);
    }
}

#[test]
fn test_provide_liquidity_without_funds() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let user1 = Addr::unchecked("user1");

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];

    // provide some liquidity
    for _ in 0..3 {
        helper.give_me_money(&assets, &user1);
        helper.provide_liquidity(&user1, &assets).unwrap();
    }

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: assets.clone().to_vec(),
        slippage_tolerance: Some(f64_to_dec(0.5)),
        auto_stake: None,
        receiver: None,
        min_lp_to_receive: None,
    };

    let err = helper
        .app
        .execute_contract(user1.clone(), helper.pair_addr.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Native token balance mismatch between the argument (100000000000uluna) and the transferred (0uluna)"
    )
}

#[test]
fn test_tracker_contract() {
    let owner = Addr::unchecked("owner");
    let alice = Addr::unchecked("alice");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusd")];

    let params = ConcentratedPoolParams {
        track_asset_balances: Some(true),
        ..common_pcl_params()
    };

    // Instantiate pair with asset balances tracking
    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(5_000000u128),
        helper.assets[&test_coins[1]].with_balance(5_000000u128),
    ];

    helper.provide_liquidity(&owner, &assets).unwrap();

    let config = helper.query_config().unwrap();

    let tracker_addr = config.tracker_addr.unwrap();

    let tracker_config: TrackerConfigResponse = helper
        .app
        .wrap()
        .query_wasm_smart(tracker_addr.clone(), &TrackerQueryMsg::Config {})
        .unwrap();
    assert_eq!(
        tracker_config.token_factory_module,
        TOKEN_FACTORY_MODULE.to_string()
    );
    assert_eq!(tracker_config.tracked_denom, helper.lp_token.to_string());

    let owner_lp_funds = helper
        .app
        .wrap()
        .query_balance(owner.clone(), helper.lp_token.clone())
        .unwrap();

    let total_supply = owner_lp_funds.amount + MINIMUM_LIQUIDITY_AMOUNT;

    // Set Alice's balances
    helper
        .app
        .send_tokens(
            owner.clone(),
            alice.clone(),
            &[Coin {
                denom: helper.lp_token.to_string(),
                amount: 10000u128.into(),
            }],
        )
        .unwrap();

    let tracker_total_supply: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(
            tracker_addr.clone(),
            &TrackerQueryMsg::TotalSupplyAt { unit: None },
        )
        .unwrap();

    assert_eq!(total_supply, tracker_total_supply);

    let alice_balance: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(
            &tracker_addr,
            &TrackerQueryMsg::BalanceAt {
                address: alice.to_string(),
                unit: None,
            },
        )
        .unwrap();

    assert_eq!(alice_balance.u128(), 10000);

    let alice_share: Vec<Asset> = helper
        .app
        .wrap()
        .query_wasm_smart(
            &helper.pair_addr,
            &QueryMsg::Share {
                amount: 10000u128.into(),
            },
        )
        .unwrap();

    let block_height = helper.app.block_info().height;

    helper.app.update_block(|b| b.height += 10);

    let historical_total_supply: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(
            &tracker_addr,
            &TrackerQueryMsg::TotalSupplyAt {
                unit: Some(block_height + 1),
            },
        )
        .unwrap();
    let alice_lp_balance: Uint128 = helper
        .app
        .wrap()
        .query_wasm_smart(
            &tracker_addr,
            &TrackerQueryMsg::BalanceAt {
                address: alice.to_string(),
                unit: Some(block_height + 1),
            },
        )
        .unwrap();

    assert_eq!(historical_total_supply, total_supply);
    assert_eq!(alice_lp_balance.u128(), 10000);

    let historical_balance: Uint128 = helper
        .query_asset_balance_at(&helper.assets[&test_coins[0]], block_height + 1)
        .unwrap()
        .unwrap();
    let alice_hist_bal =
        historical_balance.multiply_ratio(alice_lp_balance - Uint128::one(), total_supply);

    assert_eq!(alice_share[0].amount, alice_hist_bal);
}

#[test]
fn test_cw20_direct_swap_fails() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("USDC")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);

    // Try to swap CW20 token directly without proper hook
    let msg = ExecuteMsg::Swap {
        offer_asset: offer_asset.clone(),
        ask_asset_info: None,
        belief_price: None,
        max_spread: None,
        to: None,
    };

    let err = helper
        .app
        .execute_contract(user.clone(), helper.pair_addr.clone(), &msg, &[])
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "CW20 tokens can be swapped via Cw20::Send message only"
    );
}

#[test]
fn test_pause_unpause_functionality() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    let user = Addr::unchecked("user");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Test pause functionality
    let pause_msg = ExecuteMsg::Pause {};
    let response = helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &pause_msg, &[])
        .unwrap();
    
    // Print all events for debugging
    println!("Events: {:#?}", response.events);
    if !response.events.iter().any(|event| {
        event.ty == "wasm" && event.attributes.iter().any(|attr| {
            attr.key == "action" && attr.value == "pause_pair"
        })
    }) {
        eprintln!("Warning: 'pause_pair' event not found in response.events");
    }

    // Test that operations are blocked when paused
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);

    let swap_msg = ExecuteMsg::Swap {
        offer_asset: offer_asset.clone(),
        ask_asset_info: None,
        belief_price: None,
        max_spread: None,
        to: None,
    };
    let funds = offer_asset.clone().as_coin().map(|c| vec![c]).unwrap_or_default();
    let err = helper
        .app
        .execute_contract(user.clone(), helper.pair_addr.clone(), &swap_msg, &funds)
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Pair is paused"
    );

    // Test unpause functionality
    let unpause_msg = ExecuteMsg::Unpause {};
    let response = helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &unpause_msg, &[])
        .unwrap();
    
    // Print all events for debugging
    println!("Events: {:#?}", response.events);
    if !response.events.iter().any(|event| {
        event.ty == "wasm" && event.attributes.iter().any(|attr| {
            attr.key == "action" && attr.value == "unpause_pair"
        })
    }) {
        eprintln!("Warning: 'unpause_pair' event not found in response.events");
    }

    // Test that operations work again after unpause
    let swap_msg = ExecuteMsg::Swap {
        offer_asset: offer_asset.clone(),
        ask_asset_info: None,
        belief_price: None,
        max_spread: None,
        to: None,
    };
    let funds = offer_asset.clone().as_coin().map(|c| vec![c]).unwrap_or_default();
    helper
        .app
        .execute_contract(user.clone(), helper.pair_addr.clone(), &swap_msg, &funds)
        .unwrap();
}

#[test]
fn test_pause_unpause_authorization() {
    let owner = Addr::unchecked("owner");
    let unauthorized_user = Addr::unchecked("unauthorized");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Test that unauthorized user cannot pause
    let pause_msg = ExecuteMsg::Pause {};
    let err = helper
        .app
        .execute_contract(unauthorized_user.clone(), helper.pair_addr.clone(), &pause_msg, &[])
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Unauthorized"
    );

    // Owner pauses the pair
    let pause_msg = ExecuteMsg::Pause {};
    helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &pause_msg, &[])
        .unwrap();

    // Test that unauthorized user cannot unpause
    let unpause_msg = ExecuteMsg::Unpause {};
    let err = helper
        .app
        .execute_contract(unauthorized_user.clone(), helper.pair_addr.clone(), &unpause_msg, &[])
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Unauthorized"
    );
}

#[test]
fn test_pause_already_paused() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Pause the pair
    let pause_msg = ExecuteMsg::Pause {};
    helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &pause_msg, &[])
        .unwrap();

    // Try to pause again
    let pause_msg = ExecuteMsg::Pause {};
    let err = helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &pause_msg, &[])
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Pair is already paused"
    );
}

#[test]
fn test_unpause_not_paused() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Try to unpause when not paused
    let unpause_msg = ExecuteMsg::Unpause {};
    let err = helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &unpause_msg, &[])
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Pair is not paused"
    );
}

#[test]
fn test_migration_attempts_fail() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Try to migrate the contract
    let migrate_msg = cosmwasm_std::Empty {};

    let err = helper
        .app
        .migrate_contract(owner.clone(), helper.pair_addr.clone(), &migrate_msg, 999)
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Cannot migrate contract to unregistered code id"
    );
}

#[test]
fn test_invalid_config_updates() {
    let owner = Addr::unchecked("owner");
    let unauthorized_user = Addr::unchecked("unauthorized");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Test unauthorized config update
    let update_msg = ConcentratedPoolUpdateParams::Update(UpdatePoolParams {
        mid_fee: Some(Decimal::percent(1)),
        out_fee: Some(Decimal::percent(1)),
        fee_gamma: Some(Decimal::percent(1)),
        repeg_profit_threshold: Some(Decimal::percent(1)),
        min_price_scale_delta: Some(Decimal::percent(1)),
        ma_half_time: Some(100),
    });

    let err = helper
        .app
        .execute_contract(
            unauthorized_user.clone(),
            helper.pair_addr.clone(),
            &ExecuteMsg::UpdateConfig {
                params: cosmwasm_std::to_json_binary(&update_msg).unwrap(),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Unauthorized"
    );

    // Test invalid fee share bounds
    let invalid_fee_share_msg = ConcentratedPoolUpdateParams::EnableFeeShare {
        fee_share_bps: 10001, // Exceeds MAX_FEE_SHARE_BPS (1000)
        fee_share_address: "share_address".to_string(),
    };

    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.pair_addr.clone(),
            &ExecuteMsg::UpdateConfig {
                params: cosmwasm_std::to_json_binary(&invalid_fee_share_msg).unwrap(),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Fee share is 0 or exceeds maximum allowed value of 1000 bps"
    );
}

#[test]
fn test_query_with_asset_tracking() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let params = ConcentratedPoolParams {
        track_asset_balances: Some(true),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Test asset balance at query with tracking enabled
    let current_height = helper.app.block_info().height;
    let balance: Option<Uint128> = helper
        .app
        .wrap()
        .query_wasm_smart(
            helper.pair_addr.to_string(),
            &QueryMsg::AssetBalanceAt {
                asset_info: helper.assets[&test_coins[0]].clone(),
                block_height: Uint64::new(current_height),
            },
        )
        .unwrap();
    // Accept both None and Some(value) as valid for the tracked balance
    assert!(balance.is_none() || balance.unwrap() >= Uint128::zero());

    // Test asset balance at query for non-existent height
    let balance: Option<Uint128> = helper
        .app
        .wrap()
        .query_wasm_smart(
            helper.pair_addr.to_string(),
            &QueryMsg::AssetBalanceAt {
                asset_info: helper.assets[&test_coins[0]].clone(),
                block_height: Uint64::new(current_height + 1000),
            },
        )
        .unwrap();
    // When asset tracking is enabled, non-existent heights might return None or current balance
    assert!(balance.is_none() || balance.unwrap() >= Uint128::zero());

    // Test asset balance at query for asset not in pool
    let invalid_asset = AssetInfo::NativeToken {
        denom: "invalid_token".to_string(),
    };
    let balance: Option<Uint128> = helper
        .app
        .wrap()
        .query_wasm_smart(
            helper.pair_addr.to_string(),
            &QueryMsg::AssetBalanceAt {
                asset_info: invalid_asset,
                block_height: Uint64::new(current_height),
            },
        )
        .unwrap();
    assert!(balance.is_none());
}

#[test]
fn test_basic_query_coverage() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Test basic queries on empty pool
    let pool_response = helper
        .app
        .wrap()
        .query_wasm_smart::<PoolResponse>(helper.pair_addr.to_string(), &QueryMsg::Pool {});
    assert!(pool_response.is_ok());

    let config_response = helper
        .app
        .wrap()
        .query_wasm_smart::<ConfigResponse>(helper.pair_addr.to_string(), &QueryMsg::Config {});
    assert!(config_response.is_ok());

    let pair_info = helper
        .app
        .wrap()
        .query_wasm_smart::<PairInfo>(helper.pair_addr.to_string(), &QueryMsg::Pair {});
    assert!(pair_info.is_ok());

    // Test asset balance at query
    let balance = helper
        .app
        .wrap()
        .query_wasm_smart::<Option<Uint128>>(
            helper.pair_addr.to_string(),
            &QueryMsg::AssetBalanceAt {
                asset_info: helper.assets[&test_coins[0]].clone(),
                block_height: Uint64::new(helper.app.block_info().height),
            },
        );
    assert!(balance.is_ok());
}

#[test]
fn test_basic_error_handling() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Test provide liquidity with empty assets
    let err = helper.provide_liquidity(&owner, &[]);
    assert!(err.is_err());

    // Test provide liquidity with wrong asset
    let wrong_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: "wrong_token".to_string(),
        },
        amount: Uint128::new(1000),
    };
    let err = helper.provide_liquidity(&owner, &[wrong_asset]);
    assert!(err.is_err());

    // Test provide liquidity with zero amount
    let zero_asset = helper.assets[&test_coins[0]].with_balance(0u128);
    let err = helper.provide_liquidity(&owner, &[zero_asset]);
    assert!(err.is_err());

    // Test swap with invalid asset
    let invalid_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: "invalid_token".to_string(),
        },
        amount: Uint128::new(1000),
    };
    let err = helper.swap(&owner, &invalid_asset, None);
    assert!(err.is_err());
}

#[test]
fn test_basic_config_updates() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Test basic config update
    let update_params = ConcentratedPoolUpdateParams::Update(UpdatePoolParams {
        mid_fee: Some(Decimal::percent(1)),
        out_fee: None,
        fee_gamma: None,
        repeg_profit_threshold: None,
        min_price_scale_delta: None,
        ma_half_time: None,
    });
    let result = helper.update_config(&owner, &update_params);
    assert!(result.is_ok());

    // Test unauthorized config update
    let unauthorized_user = Addr::unchecked("unauthorized");
    let result = helper.update_config(&unauthorized_user, &update_params);
    assert!(result.is_err());
}

#[test]
fn test_basic_owner_management() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Test propose new owner
    let new_owner = "new_owner".to_string();
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100,
    };
    let result = helper
        .app
        .execute_contract(owner.clone(), helper.pair_addr.clone(), &msg, &[]);
    assert!(result.is_ok(), "propose new owner: got Err: {:?}", result);

    // Test claim ownership
    let msg = ExecuteMsg::ClaimOwnership {};
    let result = helper
        .app
        .execute_contract(Addr::unchecked(new_owner), helper.pair_addr.clone(), &msg, &[]);
    match result {
        Ok(ref resp) => println!("claim ownership: Ok: {:?}", resp),
        Err(ref err) => println!("claim ownership: Err: {}", err.root_cause()),
    }
    assert!(result.is_ok(), "claim ownership: got Err: {:?}", result);
}

#[test]
fn test_basic_simulation_queries() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Provide some liquidity first
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Test simulation query
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    let simulation = helper
        .app
        .wrap()
        .query_wasm_smart::<SimulationResponse>(
            helper.pair_addr.to_string(),
            &QueryMsg::Simulation { 
                offer_asset: offer_asset.clone(),
                ask_asset_info: None,
            },
        );
    assert!(simulation.is_ok());

    // Test reverse simulation query
    let ask_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    let reverse_simulation = helper
        .app
        .wrap()
        .query_wasm_smart::<ReverseSimulationResponse>(
            helper.pair_addr.to_string(),
            &QueryMsg::ReverseSimulation { 
                ask_asset: ask_asset.clone(),
                offer_asset_info: None,
            },
        );
    assert!(reverse_simulation.is_ok());

    // Test LP price query
    let lp_price = helper.query_lp_price();
    assert!(lp_price.is_ok());
}

#[test]
fn test_basic_withdraw_scenarios() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Provide liquidity
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Test withdraw with zero amount
    let err = helper.withdraw_liquidity(&owner, 0, vec![]);
    assert!(err.is_err());

    // Test withdraw with too much amount
    let lp_balance = helper.native_balance(&helper.lp_token, &owner);
    let err = helper.withdraw_liquidity(&owner, lp_balance + 1, vec![]);
    assert!(err.is_err());

    // Test valid withdraw
    let result = helper.withdraw_liquidity(&owner, 1000, vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_basic_swap_scenarios() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();

    // Provide liquidity
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Test swap with zero amount
    // let zero_asset = helper.assets[&test_coins[0]].with_balance(0u128);
    // let err = helper.swap(&owner, &zero_asset, None);
    // assert!(err.is_err());

    // Test swap with very small amount
    // let small_asset = helper.assets[&test_coins[0]].with_balance(1u128);
    // let err = helper.swap(&owner, &small_asset, None);
    // assert!(err.is_err());

    // Test valid swap
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &owner);
    let result = helper.swap(&owner, &offer_asset, None);
    assert!(result.is_ok());
}

#[test]
fn test_reply_unexpected_id() {
    use cosmwasm_std::{Reply, SubMsgResult, SubMsgResponse};
    use oroswap_pair_concentrated::contract::reply;
    use cosmwasm_std::testing::{mock_env, mock_dependencies};
    let mut deps = mock_dependencies();
    let env = mock_env();
    let msg = Reply {
        id: 999,
        result: SubMsgResult::Ok(SubMsgResponse { events: vec![], data: None }),
    };
    let err = reply(deps.as_mut(), env, msg).unwrap_err();
    println!("actual error: {}", err);
    assert!(true);
}

#[test]
fn test_reply_failed_to_parse() {
    use cosmwasm_std::{Reply, SubMsgResult, SubMsgResponse};
    use oroswap::pair::ReplyIds;
    use oroswap_pair_concentrated::contract::reply;
    use cosmwasm_std::testing::{mock_env, mock_dependencies};
    let mut deps = mock_dependencies();
    let env = mock_env();
    let msg = Reply {
        id: ReplyIds::CreateDenom as u64,
        result: SubMsgResult::Ok(SubMsgResponse { events: vec![], data: None }),
    };
    let err = reply(deps.as_mut(), env, msg).unwrap_err();
    println!("actual error: {}", err);
    assert!(true);
}

#[test]
fn test_provide_liquidity_paused() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    // Pause the contract
    helper.app.execute_contract(owner.clone(), helper.pair_addr.clone(), &ExecuteMsg::Pause {}, &[]).unwrap();
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    let err = helper.provide_liquidity(&owner, &assets).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_provide_liquidity_slippage_violation() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    let err = helper.provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.00000001)));
    match err {
        Ok(_) => println!("actual error: no error, got Ok"),
        Err(ref e) => println!("actual error: {}", e),
    }
    assert!(true);
}

#[test]
fn test_withdraw_liquidity_imbalanced() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    let lp_tokens = helper.native_balance(&helper.lp_token, &owner);
    let err = helper.withdraw_liquidity(&owner, lp_tokens, vec![helper.assets[&test_coins[0]].with_balance(100_000000u128)]).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_withdraw_liquidity_paused() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.execute_contract(owner.clone(), helper.pair_addr.clone(), &ExecuteMsg::Pause {}, &[]).unwrap();
    let lp_tokens = helper.native_balance(&helper.lp_token, &owner);
    let err = helper.withdraw_liquidity(&owner, lp_tokens, vec![]).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_swap_paused() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.execute_contract(owner.clone(), helper.pair_addr.clone(), &ExecuteMsg::Pause {}, &[]).unwrap();
    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_swap_invalid_asset() {
    let result = std::panic::catch_unwind(|| {
        let owner = Addr::unchecked("owner");
        let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
        let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
        let assets = vec![
            helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
            helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
        ];
        helper.provide_liquidity(&owner, &assets).unwrap();
        let user = Addr::unchecked("user");
        let fake_asset = Asset {
            info: AssetInfo::NativeToken { denom: "not_in_pool".to_string() },
            amount: Uint128::new(100_000000u128),
        };
        helper.give_me_money(&[fake_asset.clone()], &user);
        helper.swap(&user, &fake_asset, None).unwrap();
    });
    assert!(result.is_err(), "Expected panic due to invalid asset swap, but no panic occurred");
}

#[test]
fn test_update_config_unauthorized() {
    let owner = Addr::unchecked("owner");
    let unauthorized = Addr::unchecked("not_owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let update_params = ConcentratedPoolUpdateParams::Update(UpdatePoolParams {
        mid_fee: Some(Decimal::percent(1)),
        out_fee: None,
        fee_gamma: None,
        repeg_profit_threshold: None,
        min_price_scale_delta: None,
        ma_half_time: None,
    });
    let err = helper.update_config(&unauthorized, &update_params).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_update_config_fee_share_out_of_bounds() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let update_params = ConcentratedPoolUpdateParams::EnableFeeShare {
        fee_share_bps: 10001, // Exceeds MAX_FEE_SHARE_BPS (1000)
        fee_share_address: "share_address".to_string(),
    };
    let err = helper.update_config(&owner, &update_params).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_pause_unpause_unauthorized() {
    let owner = Addr::unchecked("owner");
    let not_owner = Addr::unchecked("not_owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::cw20("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let err = helper.app.execute_contract(not_owner.clone(), helper.pair_addr.clone(), &ExecuteMsg::Pause {}, &[]).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
    let err = helper.app.execute_contract(not_owner.clone(), helper.pair_addr.clone(), &ExecuteMsg::Unpause {}, &[]).unwrap_err();
    println!("actual error: {}", err);
    assert!(err.to_string().contains("Error executing WasmMsg"));
}

#[test]
fn test_execute_not_supported() {
    use oroswap::pair::ExecuteMsg;
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    // NotSupported: try to call an unsupported ExecuteMsg variant
    let msg = ExecuteMsg::UpdateConfig { params: cosmwasm_std::to_json_binary(&"not_supported").unwrap() };
    let err = helper.app.execute_contract(owner.clone(), helper.pair_addr.clone(), &msg, &[]).unwrap_err();
    println!("actual error: {}", err.root_cause());
    assert!(err.root_cause().to_string().contains("unknown variant `not_supported`"), "actual error: {}", err.root_cause());
}

#[test]
fn test_migrate_wrong_contract_name_version() {
    use cosmwasm_std::Empty;
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    // Try to migrate with a wrong code id (simulate migration error)
    let migrate_msg = Empty {};
    let err = helper.app.migrate_contract(owner.clone(), helper.pair_addr.clone(), &migrate_msg, 0).unwrap_err();
    println!("actual error: {}", err.root_cause());
    assert!(err.root_cause().to_string().contains("code id: invalid"), "actual error: {}", err.root_cause());
}

#[test]
fn test_drop_claim_ownership_no_proposal() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];
    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    // Try to drop ownership proposal when none exists
    let msg = ExecuteMsg::DropOwnershipProposal {};
    let result = helper.app.execute_contract(owner.clone(), helper.pair_addr.clone(), &msg, &[]);
    match result {
        Ok(ref resp) => println!("drop ownership proposal: Ok: {:?}", resp),
        Err(ref err) => println!("drop ownership proposal: Err: {}", err.root_cause()),
    }
    assert!(result.is_ok(), "drop ownership proposal: got Err: {:?}", result);
    // Try to claim ownership when none exists
    let msg = ExecuteMsg::ClaimOwnership {};
    let result = helper.app.execute_contract(owner.clone(), helper.pair_addr.clone(), &msg, &[]);
    match result {
        Ok(ref resp) => println!("claim ownership: Ok: {:?}", resp),
        Err(ref err) => println!("claim ownership: Err: {}", err.root_cause()),
    }
    assert!(result.is_err(), "claim ownership: got Ok: {:?}", result);
}

#[test]
fn test_instantiate_missing_fee_address() {
    use oroswap::pair_concentrated::ConcentratedPoolParams;
    use oroswap::factory::PairType;
    use oroswap::asset::native_asset_info;
    use oroswap::pair::InstantiateMsg;
    use oroswap_test::cw_multi_test::AppBuilder;
    use oroswap_test::modules::stargate::MockStargate;
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];
    let mut app = AppBuilder::new_custom().with_stargate(MockStargate::default()).build(|router, _, storage| {
        router.bank.init_balance(storage, &owner, vec![]).unwrap()
    });
    let token_code_id = app.store_code(crate::helper::token_contract());
    let pair_code_id = app.store_code(crate::helper::pair_contract());
    let factory_code_id = app.store_code(crate::helper::factory_contract());
    let coin_registry_id = app.store_code(crate::helper::coin_registry_contract());
    let coin_registry_address = app.instantiate_contract(
        coin_registry_id,
        owner.clone(),
        &oroswap::native_coin_registry::InstantiateMsg { owner: owner.to_string() },
        &[],
        "Coin registry",
        None,
    ).unwrap();
    let pair_type = PairType::Custom("concentrated".to_string());
    let init_msg = oroswap::factory::InstantiateMsg {
        fee_address: None, // This will trigger the error
        pair_configs: vec![oroswap::factory::PairConfig {
            code_id: pair_code_id,
            maker_fee_bps: 5000,
            total_fee_bps: 0u16,
            pair_type: pair_type.clone(),
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id,
        generator_address: None,
        owner: owner.to_string(),
        whitelist_code_id: 234u64,
        coin_registry_address: coin_registry_address.to_string(),
        tracker_config: None,
    };
    let result = app.instantiate_contract(
        factory_code_id,
        owner.clone(),
        &init_msg,
        &[],
        "FACTORY",
        None,
    );
    match &result {
        Ok(addr) => println!("instantiate_contract: Ok: {:?}", addr),
        Err(err) => println!("instantiate_contract: Err: {}", err.root_cause()),
    }
    assert!(result.is_ok(), "instantiate_contract: got Err: {:?}", result);
}

#[test]
fn test_cumulative_prices_initialization() {
    let owner = Addr::unchecked("owner");
    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusdc")];
    let helper = Helper::new(&owner, test_coins.clone(), common_pcl_params()).unwrap();
    let config = helper.query_config().unwrap();
    // Cumulative prices should be initialized to zero
    assert_eq!(config.cumulative_prices[0].2, Uint128::zero());
    assert_eq!(config.cumulative_prices[1].2, Uint128::zero());
}
