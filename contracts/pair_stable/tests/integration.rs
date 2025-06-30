#![cfg(not(tarpaulin_include))]

use oroswap::asset::{native_asset_info, Asset, AssetInfo, PairInfo};
use oroswap::factory::{
    ExecuteMsg as FactoryExecuteMsg, InstantiateMsg as FactoryInstantiateMsg, PairConfig, PairType,
    QueryMsg as FactoryQueryMsg,
};
use oroswap::observation::OracleObservation;
use oroswap::pair::{
    ConfigResponse, CumulativePricesResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
    PoolResponse, QueryMsg, StablePoolConfig, StablePoolParams, StablePoolUpdateParams,
    MAX_FEE_SHARE_BPS, TWAP_PRECISION,
};

use oroswap_pair_stable::error::ContractError;

use std::str::FromStr;

use oroswap::common::LP_SUBDENOM;
use oroswap::token::InstantiateMsg as TokenInstantiateMsg;
use oroswap_pair_stable::math::{MAX_AMP, MAX_AMP_CHANGE, MIN_AMP_CHANGING_TIME};
use oroswap_test::cw_multi_test::{AppBuilder, ContractWrapper, Executor};
use oroswap_test::modules::stargate::{MockStargate, StargateApp as TestApp};
use cosmwasm_std::{
    attr, coin, from_json, to_json_binary, Addr, Coin, Decimal, QueryRequest, Uint128, WasmQuery,
};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};

const OWNER: &str = "owner";

mod helper;

fn mock_app(owner: Addr, coins: Vec<Coin>) -> TestApp {
    AppBuilder::new_custom()
        .with_stargate(MockStargate::default())
        .build(|router, _, storage| {
            // initialization moved to App construction
            router.bank.init_balance(storage, &owner, coins).unwrap()
        })
}

fn store_token_code(app: &mut TestApp) -> u64 {
    let oro_token_contract = Box::new(ContractWrapper::new_with_empty(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    app.store_code(oro_token_contract)
}

fn store_pair_code(app: &mut TestApp) -> u64 {
    let pair_contract = Box::new(
        ContractWrapper::new_with_empty(
            oroswap_pair_stable::contract::execute,
            oroswap_pair_stable::contract::instantiate,
            oroswap_pair_stable::contract::query,
        )
        .with_reply_empty(oroswap_pair_stable::contract::reply),
    );

    app.store_code(pair_contract)
}

fn store_factory_code(app: &mut TestApp) -> u64 {
    let factory_contract = Box::new(
        ContractWrapper::new_with_empty(
            oroswap_factory::contract::execute,
            oroswap_factory::contract::instantiate,
            oroswap_factory::contract::query,
        )
        .with_reply_empty(oroswap_factory::contract::reply),
    );

    app.store_code(factory_contract)
}

fn store_coin_registry_code(app: &mut TestApp) -> u64 {
    let coin_registry_contract = Box::new(ContractWrapper::new_with_empty(
        oroswap_native_coin_registry::contract::execute,
        oroswap_native_coin_registry::contract::instantiate,
        oroswap_native_coin_registry::contract::query,
    ));

    app.store_code(coin_registry_contract)
}

fn store_generator_code(app: &mut TestApp) -> u64 {
    let generator_contract = Box::new(ContractWrapper::new_with_empty(
        oroswap_incentives::execute::execute,
        oroswap_incentives::instantiate::instantiate,
        oroswap_incentives::query::query,
    ));

    app.store_code(generator_contract)
}

fn instantiate_coin_registry(mut app: &mut TestApp, coins: Option<Vec<(String, u8)>>) -> Addr {
    let coin_registry_id = store_coin_registry_code(&mut app);
    let coin_registry_address = app
        .instantiate_contract(
            coin_registry_id,
            Addr::unchecked(OWNER),
            &oroswap::native_coin_registry::InstantiateMsg {
                owner: OWNER.to_string(),
            },
            &[],
            "Coin registry",
            None,
        )
        .unwrap();

    if let Some(coins) = coins {
        app.execute_contract(
            Addr::unchecked(OWNER),
            coin_registry_address.clone(),
            &oroswap::native_coin_registry::ExecuteMsg::Add {
                native_coins: coins,
            },
            &[],
        )
        .unwrap();
    }

    coin_registry_address
}

fn instantiate_pair(mut router: &mut TestApp, owner: &Addr) -> Addr {
    let coin_registry_address = instantiate_coin_registry(
        &mut router,
        Some(vec![("uusd".to_string(), 6), ("uluna".to_string(), 6)]),
    );

    let token_code_id = store_token_code(&mut router);
    let pair_code_id = store_pair_code(&mut router);
    let factory_code_id = store_factory_code(&mut router);

    let factory_init_msg = FactoryInstantiateMsg {
        fee_address: Some("fee_address".to_string()),
        pair_configs: vec![PairConfig {
            code_id: pair_code_id,
            maker_fee_bps: 5000,
            total_fee_bps: 5u16,
            pair_type: PairType::Stable {},
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

    let factory_instance = router
        .instantiate_contract(
            factory_code_id,
            owner.clone(),
            &factory_init_msg,
            &[],
            "FACTORY",
            None,
        )
        .unwrap();

    // Create pair through factory
    let msg = FactoryExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 100,
                owner: None,
            })
            .unwrap(),
        ),
    };

    router
        .execute_contract(
            owner.clone(),
            factory_instance.clone(),
            &msg,
            &[Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            }],
        )
        .unwrap();

    // Query the created pair
    let msg = FactoryQueryMsg::Pair {
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
        pair_type: PairType::Stable {},
    };

    let res: PairInfo = router
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();

    res.contract_addr
}

#[test]
fn test_provide_and_withdraw_liquidity() {
    let owner = Addr::unchecked("owner");
    let alice_address = Addr::unchecked("alice");

    let mut router = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    );

    // Set Alice's balances
    router
        .send_tokens(
            owner.clone(),
            alice_address.clone(),
            &[
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::new(533_000u128),
                },
                Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::new(500_000u128),
                },
            ],
        )
        .unwrap();

    // Init pair
    let pair_instance = instantiate_pair(&mut router, &owner);

    let res: PairInfo = router
        .wrap()
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pair_instance.to_string(),
            msg: to_json_binary(&QueryMsg::Pair {}).unwrap(),
        }))
        .unwrap();

    let lp_token = res.liquidity_token;

    assert_eq!(
        res.asset_infos,
        [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
    );

    // Try to provide liquidity less then MINIMUM_LIQUIDITY_AMOUNT
    let (msg, coins) = provide_liquidity_msg(Uint128::new(100), Uint128::new(100), None, None);
    let err = router
        .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap_err();
    assert_eq!(
        "Initial liquidity must be more than 1000",
        err.root_cause().to_string()
    );

    // Try to provide liquidity equal to MINIMUM_LIQUIDITY_AMOUNT
    let (msg, coins) = provide_liquidity_msg(Uint128::new(500), Uint128::new(500), None, None);
    let err = router
        .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap_err();
    assert_eq!(
        "Initial liquidity must be more than 1000",
        err.root_cause().to_string()
    );

    // Provide liquidity
    let (msg, coins) =
        provide_liquidity_msg(Uint128::new(100000), Uint128::new(100000), None, None);
    let res = router
        .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap();

    assert_eq!(
        res.events[1].attributes[1],
        attr("action", "provide_liquidity")
    );
    assert_eq!(res.events[1].attributes[3], attr("receiver", "alice"),);
    assert_eq!(
        res.events[1].attributes[4],
        attr("assets", "100000uusd, 100000uluna")
    );
    assert_eq!(
        res.events[1].attributes[5],
        attr("share", 199000u128.to_string())
    );

    // Provide with min_lp_to_receive with a bigger amount than expected.
    let min_lp_amount_to_receive: Uint128 = router
        .wrap()
        .query_wasm_smart(
            pair_instance.clone(),
            &QueryMsg::SimulateProvide {
                assets: vec![
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                        amount: Uint128::new(100000),
                    },
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(100000),
                    },
                ],
                slippage_tolerance: None,
            },
        )
        .unwrap();

    let double_amount_to_receive = min_lp_amount_to_receive * Uint128::new(2);

    let (msg, coins) = provide_liquidity_msg(
        Uint128::new(100000),
        Uint128::new(100000),
        None,
        Some(double_amount_to_receive.clone()),
    );

    let err = router
        .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap_err();

    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ProvideSlippageViolation(Uint128::new(200000), double_amount_to_receive)
    );

    // Provide with min_lp_to_receive with amount expected
    let min_lp_amount_to_receive: Uint128 = router
        .wrap()
        .query_wasm_smart(
            pair_instance.clone(),
            &QueryMsg::SimulateProvide {
                assets: vec![
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                        amount: Uint128::new(100000),
                    },
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uluna".to_string(),
                        },
                        amount: Uint128::new(100000),
                    },
                ],
                slippage_tolerance: None,
            },
        )
        .unwrap();

    let (msg, coins) = provide_liquidity_msg(
        Uint128::new(100000),
        Uint128::new(100000),
        None,
        Some(min_lp_amount_to_receive),
    );

    router
        .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap();

    // Provide liquidity for a custom receiver
    let (msg, coins) = provide_liquidity_msg(
        Uint128::new(100000),
        Uint128::new(100000),
        Some("bob".to_string()),
        None,
    );
    let res = router
        .execute_contract(alice_address.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap();

    assert_eq!(
        res.events[1].attributes[1],
        attr("action", "provide_liquidity")
    );
    assert_eq!(res.events[1].attributes[3], attr("receiver", "bob"),);
    assert_eq!(
        res.events[1].attributes[4],
        attr("assets", "100000uusd, 100000uluna")
    );
    assert_eq!(
        res.events[1].attributes[5],
        attr("share", 200000u128.to_string())
    );

    // Withdraw liquidity doubling the minimum to recieve
    let min_assets_to_receive: Vec<Asset> = router
        .wrap()
        .query_wasm_smart(
            pair_instance.clone(),
            &QueryMsg::SimulateWithdraw {
                lp_amount: Uint128::new(100),
            },
        )
        .unwrap();

    let err = router
        .execute_contract(
            alice_address.clone(),
            pair_instance.clone(),
            &ExecuteMsg::WithdrawLiquidity {
                assets: vec![],
                min_assets_to_receive: Some(
                    min_assets_to_receive
                        .iter()
                        .map(|a| Asset {
                            info: a.info.clone(),
                            amount: a.amount * Uint128::new(2),
                        })
                        .collect(),
                ),
            },
            &[coin(100u128, lp_token.clone())],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::WithdrawSlippageViolation {
            asset_name: "uusd".to_string(),
            expected: Uint128::new(98),
            received: Uint128::new(49)
        }
    );

    // Withdraw liquidity with minimum to receive

    let min_assets_to_receive: Vec<Asset> = router
        .wrap()
        .query_wasm_smart(
            pair_instance.clone(),
            &QueryMsg::SimulateWithdraw {
                lp_amount: Uint128::new(100),
            },
        )
        .unwrap();

    router
        .execute_contract(
            alice_address.clone(),
            pair_instance.clone(),
            &ExecuteMsg::WithdrawLiquidity {
                assets: vec![],
                min_assets_to_receive: Some(min_assets_to_receive),
            },
            &[coin(100u128, lp_token.clone())],
        )
        .unwrap();

    // Withdraw with LP token is successful
    router
        .execute_contract(
            alice_address.clone(),
            pair_instance.clone(),
            &ExecuteMsg::WithdrawLiquidity {
                assets: vec![],
                min_assets_to_receive: None,
            },
            &[coin(50u128, lp_token.clone())],
        )
        .unwrap();
}

fn provide_liquidity_msg(
    uusd_amount: Uint128,
    uluna_amount: Uint128,
    receiver: Option<String>,
    min_lp_to_receive: Option<Uint128>,
) -> (ExecuteMsg, [Coin; 2]) {
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: uusd_amount.clone(),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: uluna_amount.clone(),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver,
        min_lp_to_receive,
    };

    let coins = [
        Coin {
            denom: "uluna".to_string(),
            amount: uluna_amount.clone(),
        },
        Coin {
            denom: "uusd".to_string(),
            amount: uusd_amount.clone(),
        },
    ];

    (msg, coins)
}

#[test]
fn provide_lp_for_single_token() {
    let owner = Addr::unchecked(OWNER);
    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    );

    let token_code_id = store_token_code(&mut app);

    let x_amount = Uint128::new(9_000_000_000_000_000);
    let y_amount = Uint128::new(9_000_000_000_000_000);
    let x_offer = Uint128::new(1_000_000_000_000_000);
    let swap_amount = Uint128::new(120_000_000);

    let token_name = "Xtoken";

    let init_msg = TokenInstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: 6,
        initial_balances: vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: x_amount,
        }],
        mint: Some(MinterResponse {
            minter: String::from(OWNER),
            cap: None,
        }),
        marketing: None,
    };

    let token_x_instance = app
        .instantiate_contract(
            token_code_id,
            owner.clone(),
            &init_msg,
            &[],
            token_name,
            None,
        )
        .unwrap();

    let token_name = "Ytoken";

    let init_msg = TokenInstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: 6,
        initial_balances: vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: y_amount,
        }],
        mint: Some(MinterResponse {
            minter: String::from(OWNER),
            cap: None,
        }),
        marketing: None,
    };

    let token_y_instance = app
        .instantiate_contract(
            token_code_id,
            owner.clone(),
            &init_msg,
            &[],
            token_name,
            None,
        )
        .unwrap();

    let pair_code_id = store_pair_code(&mut app);
    let factory_code_id = store_factory_code(&mut app);

    let init_msg = FactoryInstantiateMsg {
        fee_address: Some("fee_address".to_string()),
        pair_configs: vec![PairConfig {
            code_id: pair_code_id,
            maker_fee_bps: 0,
            total_fee_bps: 0,
            pair_type: PairType::Stable {},
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id,
        generator_address: Some(String::from("generator")),
        owner: String::from("owner0000"),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let factory_instance = app
        .instantiate_contract(
            factory_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "FACTORY",
            None,
        )
        .unwrap();

    let msg = FactoryExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::Token {
                contract_addr: token_x_instance.clone(),
            },
            AssetInfo::Token {
                contract_addr: token_y_instance.clone(),
            },
        ],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 100,
                owner: None,
            })
            .unwrap(),
        ),
    };

    app.execute_contract(owner.clone(), factory_instance.clone(), &msg, &[Coin {
        denom: "uzig".to_string(),
        amount: Uint128::new(1000), // Pool creation fee
    }])
        .unwrap();

    let msg = FactoryQueryMsg::Pair {
        asset_infos: vec![
            AssetInfo::Token {
                contract_addr: token_x_instance.clone(),
            },
            AssetInfo::Token {
                contract_addr: token_y_instance.clone(),
            },
        ],
        pair_type: PairType::Stable {},
    };

    let res: PairInfo = app
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();

    let pair_instance = res.contract_addr;

    let msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: pair_instance.to_string(),
        expires: None,
        amount: x_amount,
    };

    app.execute_contract(owner.clone(), token_x_instance.clone(), &msg, &[])
        .unwrap();

    let msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: pair_instance.to_string(),
        expires: None,
        amount: y_amount,
    };

    app.execute_contract(owner.clone(), token_y_instance.clone(), &msg, &[])
        .unwrap();

    let swap_msg = Cw20ExecuteMsg::Send {
        contract: pair_instance.to_string(),
        msg: to_json_binary(&Cw20HookMsg::Swap {
            ask_asset_info: None,
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
        amount: swap_amount,
    };

    let err = app
        .execute_contract(owner.clone(), token_x_instance.clone(), &swap_msg, &[])
        .unwrap_err();
    assert_eq!(
        "Generic error: One of the pools is empty",
        err.root_cause().to_string()
    );

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_x_instance.clone(),
                },
                amount: x_offer,
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_y_instance.clone(),
                },
                amount: Uint128::zero(),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
        min_lp_to_receive: None,
    };

    let err = app
        .execute_contract(owner.clone(), pair_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        "It is not possible to provide liquidity with one token for an empty pool",
        err.root_cause().to_string()
    );

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_x_instance.clone(),
                },
                amount: Uint128::new(1_000_000_000_000_000),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_y_instance.clone(),
                },
                amount: Uint128::new(1_000_000_000_000_000),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
        min_lp_to_receive: None,
    };

    app.execute_contract(owner.clone(), pair_instance.clone(), &msg, &[])
        .unwrap();

    // try to provide for single token and increase the ratio in the pool from 1 to 1.5
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_x_instance.clone(),
                },
                amount: Uint128::new(500_000_000_000_000),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_y_instance.clone(),
                },
                amount: Uint128::zero(),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
        min_lp_to_receive: None,
    };

    app.execute_contract(owner.clone(), pair_instance.clone(), &msg, &[])
        .unwrap();

    // try swap 120_000_000 from token_y to token_x (from lower token amount to higher)
    app.execute_contract(owner.clone(), token_y_instance.clone(), &swap_msg, &[])
        .unwrap();

    // try swap 120_000_000 from token_x to token_y (from higher token amount to lower )
    app.execute_contract(owner.clone(), token_x_instance.clone(), &swap_msg, &[])
        .unwrap();

    // try to provide for single token and increase the ratio in the pool from 1 to 2.5
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_x_instance.clone(),
                },
                amount: Uint128::new(1_000_000_000_000_000),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_y_instance.clone(),
                },
                amount: Uint128::zero(),
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
        min_lp_to_receive: None,
    };

    app.execute_contract(owner.clone(), pair_instance.clone(), &msg, &[])
        .unwrap();

    // try swap 120_000_000 from token_y to token_x (from lower token amount to higher)
    let msg = Cw20ExecuteMsg::Send {
        contract: pair_instance.to_string(),
        msg: to_json_binary(&Cw20HookMsg::Swap {
            ask_asset_info: None,
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
        amount: swap_amount,
    };

    app.execute_contract(owner.clone(), token_y_instance.clone(), &msg, &[])
        .unwrap();

    // try swap 120_000_000 from token_x to token_y (from higher token amount to lower )
    let msg = Cw20ExecuteMsg::Send {
        contract: pair_instance.to_string(),
        msg: to_json_binary(&Cw20HookMsg::Swap {
            ask_asset_info: None,
            belief_price: None,
            max_spread: None,
            to: None,
        })
        .unwrap(),
        amount: swap_amount,
    };

    let err = app
        .execute_contract(owner.clone(), token_x_instance.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Operation exceeds max spread limit"
    );
}

#[test]
fn test_compatibility_of_tokens_with_different_precision() {
    let owner = Addr::unchecked(OWNER);
    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    );

    let token_code_id = store_token_code(&mut app);

    let x_amount = Uint128::new(1000000_00000);
    let y_amount = Uint128::new(1000000_0000000);
    let x_offer = Uint128::new(1_00000);
    let y_expected_return = Uint128::new(9995000);

    let token_name = "Xtoken";

    let init_msg = TokenInstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: 5,
        initial_balances: vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: x_amount + x_offer,
        }],
        mint: Some(MinterResponse {
            minter: String::from(OWNER),
            cap: None,
        }),
        marketing: None,
    };

    let token_x_instance = app
        .instantiate_contract(
            token_code_id,
            owner.clone(),
            &init_msg,
            &[],
            token_name,
            None,
        )
        .unwrap();

    let token_name = "Ytoken";

    let init_msg = TokenInstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: 7,
        initial_balances: vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: y_amount,
        }],
        mint: Some(MinterResponse {
            minter: String::from(OWNER),
            cap: None,
        }),
        marketing: None,
    };

    let token_y_instance = app
        .instantiate_contract(
            token_code_id,
            owner.clone(),
            &init_msg,
            &[],
            token_name,
            None,
        )
        .unwrap();

    let pair_code_id = store_pair_code(&mut app);
    let factory_code_id = store_factory_code(&mut app);

    let init_msg = FactoryInstantiateMsg {
        fee_address: Some("fee_address".to_string()),
        pair_configs: vec![PairConfig {
            code_id: pair_code_id,
            maker_fee_bps: 5000,
            total_fee_bps: 5u16,
            pair_type: PairType::Stable {},
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id,
        generator_address: Some(String::from("generator")),
        owner: owner.to_string(),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let factory_instance = app
        .instantiate_contract(
            factory_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "FACTORY",
            None,
        )
        .unwrap();

    let msg = FactoryExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::Token {
                contract_addr: token_x_instance.clone(),
            },
            AssetInfo::Token {
                contract_addr: token_y_instance.clone(),
            },
        ],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 100,
                owner: None,
            })
            .unwrap(),
        ),
    };

    app.execute_contract(owner.clone(), factory_instance.clone(), &msg, &[Coin {
        denom: "uzig".to_string(),
        amount: Uint128::new(1000), // Pool creation fee
    }])
        .unwrap();

    let msg = FactoryQueryMsg::Pair {
        asset_infos: vec![
            AssetInfo::Token {
                contract_addr: token_x_instance.clone(),
            },
            AssetInfo::Token {
                contract_addr: token_y_instance.clone(),
            },
        ],
        pair_type: PairType::Stable {},
    };

    let res: PairInfo = app
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();

    let pair_instance = res.contract_addr;

    let msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: pair_instance.to_string(),
        expires: None,
        amount: x_amount + x_offer,
    };

    app.execute_contract(owner.clone(), token_x_instance.clone(), &msg, &[])
        .unwrap();

    let msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: pair_instance.to_string(),
        expires: None,
        amount: y_amount,
    };

    app.execute_contract(owner.clone(), token_y_instance.clone(), &msg, &[])
        .unwrap();

    let msg = ExecuteMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_x_instance.clone(),
                },
                amount: x_amount,
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: token_y_instance.clone(),
                },
                amount: y_amount,
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
        min_lp_to_receive: None,
    };

    app.execute_contract(owner.clone(), pair_instance.clone(), &msg, &[])
        .unwrap();

    let d: u128 = app
        .wrap()
        .query_wasm_smart(&pair_instance, &QueryMsg::QueryComputeD {})
        .unwrap();
    assert_eq!(d, 20000000000000);

    let user = Addr::unchecked("user");

    let msg = Cw20ExecuteMsg::Send {
        contract: pair_instance.to_string(),
        msg: to_json_binary(&Cw20HookMsg::Swap {
            ask_asset_info: None,
            belief_price: None,
            max_spread: None,
            to: Some(user.to_string()),
        })
        .unwrap(),
        amount: x_offer,
    };

    app.execute_contract(owner.clone(), token_x_instance.clone(), &msg, &[])
        .unwrap();

    let msg = Cw20QueryMsg::Balance {
        address: user.to_string(),
    };

    let res: BalanceResponse = app
        .wrap()
        .query_wasm_smart(&token_y_instance, &msg)
        .unwrap();

    assert_eq!(res.balance, y_expected_return);

    let d: u128 = app
        .wrap()
        .query_wasm_smart(&pair_instance, &QueryMsg::QueryComputeD {})
        .unwrap();
    assert_eq!(d, 20000000002499);
}

#[test]
fn test_if_twap_is_calculated_correctly_when_pool_idles() {
    let owner = Addr::unchecked(OWNER);
    let mut app = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000_000_000000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(100_000_000_000000u128),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    );

    let user1 = Addr::unchecked("user1");

    // Set User1's balances
    app.send_tokens(
        owner.clone(),
        user1.clone(),
        &[
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(4666666_000000),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(2000000_000000),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    )
    .unwrap();

    // Instantiate pair
    let coin_registry_address = instantiate_coin_registry(
        &mut app,
        Some(vec![("uusd".to_string(), 6), ("uluna".to_string(), 6)]),
    );
    let token_code_id = store_token_code(&mut app);
    let pair_code_id = store_pair_code(&mut app);
    let factory_code_id = store_factory_code(&mut app);
    let factory_init_msg = FactoryInstantiateMsg {
        fee_address: Some("fee_address".to_string()),
        pair_configs: vec![PairConfig {
            code_id: pair_code_id,
            maker_fee_bps: 5000,
            total_fee_bps: 5u16,
            pair_type: PairType::Stable {},
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id,
        generator_address: None,
        owner: user1.to_string(),
        whitelist_code_id: 234u64,
        coin_registry_address: coin_registry_address.to_string(),
        tracker_config: None,
    };
    let factory_addr = app
        .instantiate_contract(
            factory_code_id,
            user1.clone(),
            &factory_init_msg,
            &[],
            "FACTORY",
            None,
        )
        .unwrap();

    // Create pair through factory
    let msg = FactoryExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 100,
                owner: None,
            })
            .unwrap(),
        ),
    };

    app.execute_contract(
        user1.clone(),
        factory_addr.clone(),
        &msg,
        &[Coin {
            denom: "uzig".to_string(),
            amount: Uint128::new(1000),
        }],
    )
    .unwrap();

    // Query the created pair
    let msg = FactoryQueryMsg::Pair {
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
        pair_type: PairType::Stable {},
    };

    let res: PairInfo = app
        .wrap()
        .query_wasm_smart(&factory_addr, &msg)
        .unwrap();

    let pair_instance = res.contract_addr;

    // Provide liquidity, accumulators are empty
    let (msg, coins) = provide_liquidity_msg(
        Uint128::new(1000000_000000),
        Uint128::new(1000000_000000),
        None,
        None,
    );
    app.execute_contract(user1.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap();

    const BLOCKS_PER_DAY: u64 = 17280;
    const ELAPSED_SECONDS: u64 = BLOCKS_PER_DAY * 5;

    // A day later
    app.update_block(|b| {
        b.height += BLOCKS_PER_DAY;
        b.time = b.time.plus_seconds(ELAPSED_SECONDS);
    });

    // Provide liquidity, accumulators firstly filled with the same prices
    let (msg, coins) = provide_liquidity_msg(
        Uint128::new(3000000_000000),
        Uint128::new(1000000_000000),
        None,
        None,
    );
    app.execute_contract(user1.clone(), pair_instance.clone(), &msg, &coins)
        .unwrap();

    // Get current TWAP accumulator values
    let msg = QueryMsg::CumulativePrices {};
    let cpr_old: CumulativePricesResponse =
        app.wrap().query_wasm_smart(&pair_instance, &msg).unwrap();

    // A day later
    app.update_block(|b| {
        b.height += BLOCKS_PER_DAY;
        b.time = b.time.plus_seconds(ELAPSED_SECONDS);
    });

    // Get current twap accumulator values
    let msg = QueryMsg::CumulativePrices {};
    let cpr_new: CumulativePricesResponse =
        app.wrap().query_wasm_smart(&pair_instance, &msg).unwrap();

    let twap0 = cpr_new.cumulative_prices[0].2 - cpr_old.cumulative_prices[0].2;
    let twap1 = cpr_new.cumulative_prices[1].2 - cpr_old.cumulative_prices[1].2;

    // Prices weren't changed for the last day, uusd amount in pool = 4000000_000000, uluna = 2000000_000000
    let price_precision = Uint128::from(10u128.pow(TWAP_PRECISION.into()));
    assert_eq!(twap0 / price_precision, Uint128::new(85684)); // 1.008356286 * ELAPSED_SECONDS (86400)
    assert_eq!(twap1 / price_precision, Uint128::new(87121)); // 0.991712963 * ELAPSED_SECONDS
}

#[test]
fn create_pair_with_same_assets() {
    let owner = Addr::unchecked(OWNER);
    let mut router = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    );

    let token_code_id = store_token_code(&mut router);
    let pair_code_id = store_pair_code(&mut router);

    let msg = InstantiateMsg {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        ],
        token_code_id,
        factory_addr: String::from("factory"),
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 100,
                owner: None,
            })
            .unwrap(),
        ),
    };

    let resp = router
        .instantiate_contract(
            pair_code_id,
            owner.clone(),
            &msg,
            &[Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            }],
            String::from("PAIR"),
            None,
        )
        .unwrap_err();

    assert_eq!(
        resp.root_cause().to_string(),
        "Doubling assets in asset infos"
    )
}

#[test]
fn update_pair_config() {
    let owner = Addr::unchecked(OWNER);
    let mut router = mock_app(
        owner.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(100_000_000_000u128),
            },
            Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            },
        ],
    );

    let coin_registry_address = instantiate_coin_registry(
        &mut router,
        Some(vec![("uusd".to_string(), 6), ("uluna".to_string(), 6)]),
    );

    let token_code_id = store_token_code(&mut router);
    let pair_code_id = store_pair_code(&mut router);

    let factory_code_id = store_factory_code(&mut router);

    let init_msg = FactoryInstantiateMsg {
        fee_address: Some("fee_address".to_string()),
        pair_configs: vec![PairConfig {
            code_id: pair_code_id,
            maker_fee_bps: 5000,
            total_fee_bps: 5u16,
            pair_type: PairType::Stable {},
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id,
        generator_address: Some(String::from("generator")),
        owner: owner.to_string(),
        whitelist_code_id: 234u64,
        coin_registry_address: coin_registry_address.to_string(),
        tracker_config: None,
    };

    let factory_instance = router
        .instantiate_contract(
            factory_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "FACTORY",
            None,
        )
        .unwrap();

    // Create pair through factory
    let msg = FactoryExecuteMsg::CreatePair {
        pair_type: PairType::Stable {},
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
        init_params: Some(
            to_json_binary(&StablePoolParams {
                amp: 100,
                owner: None,
            })
            .unwrap(),
        ),
    };

    router
        .execute_contract(
            owner.clone(),
            factory_instance.clone(),
            &msg,
            &[Coin {
                denom: "uzig".to_string(),
                amount: Uint128::new(1000), // Pool creation fee
            }],
        )
        .unwrap();

    // Query the created pair
    let msg = FactoryQueryMsg::Pair {
        asset_infos: vec![
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
        ],
        pair_type: PairType::Stable {},
    };

    let res: PairInfo = router
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();

    let pair = res.contract_addr;

    let res: ConfigResponse = router
        .wrap()
        .query_wasm_smart(pair.clone(), &QueryMsg::Config {})
        .unwrap();

    let params: StablePoolConfig = from_json(&res.params.unwrap()).unwrap();

    assert_eq!(params.amp, Decimal::from_ratio(100u32, 1u32));
}
