use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{from_json, to_json_binary, Addr, Coin, ReplyOn, SubMsg, Uint128, WasmMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use oroswap::asset::{native_asset_info, AssetInfo};
use oroswap::factory::PairType;
use oroswap::router::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg,
    SimulateSwapOperationsResponse, SwapOperation, MAX_SWAP_OPERATIONS,
    RouterStage,
};

use crate::contract::{execute, instantiate, query, AFTER_SWAP_REPLY_ID};
use crate::error::ContractError;
use crate::testing::mock_querier::mock_dependencies;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        oroswap_factory: String::from("oroswapfactory"),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // It worked, let's query the state
    let config: ConfigResponse =
        from_json(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!("oroswapfactory", config.oroswap_factory.as_str());
}

#[test]
fn execute_swap_operations() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        oroswap_factory: String::from("oroswapfactory"),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::ExecuteSwapOperations {
        operations: vec![],
        minimum_receive: None,
        to: None,
        max_spread: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::MustProvideOperations {});

    let msg = ExecuteMsg::ExecuteSwapOperations {
        operations: vec![
            SwapOperation::OroSwap {
                offer_asset_info: AssetInfo::NativeToken {
                    denom: "ukrw".to_string(),
                },
                ask_asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
                pair_type: PairType::Xyk {},
            },
            SwapOperation::OroSwap {
                offer_asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
                ask_asset_info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                pair_type: PairType::Xyk {},
            },
            SwapOperation::OroSwap {
                offer_asset_info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                ask_asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0002"),
                },
                pair_type: PairType::Xyk {},
            },
        ],
        minimum_receive: Some(Uint128::from(1000000u128)),
        to: None,
        max_spread: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from(MOCK_CONTRACT_ADDR),
                    funds: vec![],
                    msg: to_json_binary(&ExecuteMsg::ExecuteSwapOperation {
                        operation: SwapOperation::OroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                            ask_asset_info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("asset0001"),
                            },
                            pair_type: PairType::Xyk {},
                        },
                        to: None,
                        max_spread: None,
                        single: false
                    })
                    .unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            },
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from(MOCK_CONTRACT_ADDR),
                    funds: vec![],
                    msg: to_json_binary(&ExecuteMsg::ExecuteSwapOperation {
                        operation: SwapOperation::OroSwap {
                            offer_asset_info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("asset0001"),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            pair_type: PairType::Xyk {},
                        },
                        to: None,
                        max_spread: None,
                        single: false
                    })
                    .unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            },
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from(MOCK_CONTRACT_ADDR),
                    funds: vec![],
                    msg: to_json_binary(&ExecuteMsg::ExecuteSwapOperation {
                        operation: SwapOperation::OroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            ask_asset_info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("asset0002"),
                            },
                            pair_type: PairType::Xyk {},
                        },
                        to: Some(String::from("addr0000")),
                        max_spread: None,
                        single: false
                    })
                    .unwrap(),
                }
                .into(),
                id: AFTER_SWAP_REPLY_ID,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            }
        ]
    );

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: String::from("addr0000"),
        amount: Uint128::from(1000000u128),
        msg: to_json_binary(&Cw20HookMsg::ExecuteSwapOperations {
            operations: vec![
                SwapOperation::OroSwap {
                    offer_asset_info: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0001"),
                    },
                    pair_type: PairType::Xyk {},
                },
                SwapOperation::OroSwap {
                    offer_asset_info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0001"),
                    },
                    ask_asset_info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                    pair_type: PairType::Xyk {},
                },
                SwapOperation::OroSwap {
                    offer_asset_info: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                    ask_asset_info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0002"),
                    },
                    pair_type: PairType::Xyk {},
                },
            ],
            minimum_receive: Some(Uint128::from(1000000u128)),
            to: Some(String::from("addr0002")),
            max_spread: None,
        })
        .unwrap(),
    });

    let env = mock_env();
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from(MOCK_CONTRACT_ADDR),
                    funds: vec![],
                    msg: to_json_binary(&ExecuteMsg::ExecuteSwapOperation {
                        operation: SwapOperation::OroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "ukrw".to_string(),
                            },
                            ask_asset_info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("asset0001"),
                            },
                            pair_type: PairType::Xyk {},
                        },
                        to: None,
                        max_spread: None,
                        single: false
                    })
                    .unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            },
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from(MOCK_CONTRACT_ADDR),
                    funds: vec![],
                    msg: to_json_binary(&ExecuteMsg::ExecuteSwapOperation {
                        operation: SwapOperation::OroSwap {
                            offer_asset_info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("asset0001"),
                            },
                            ask_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            pair_type: PairType::Xyk {},
                        },
                        to: None,
                        max_spread: None,
                        single: false
                    })
                    .unwrap(),
                }
                .into(),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            },
            SubMsg {
                msg: WasmMsg::Execute {
                    contract_addr: String::from(MOCK_CONTRACT_ADDR),
                    funds: vec![],
                    msg: to_json_binary(&ExecuteMsg::ExecuteSwapOperation {
                        operation: SwapOperation::OroSwap {
                            offer_asset_info: AssetInfo::NativeToken {
                                denom: "uluna".to_string(),
                            },
                            ask_asset_info: AssetInfo::Token {
                                contract_addr: Addr::unchecked("asset0002"),
                            },
                            pair_type: PairType::Xyk {},
                        },
                        to: Some(String::from("addr0002")),
                        max_spread: None,
                        single: false
                    })
                    .unwrap(),
                }
                .into(),
                id: AFTER_SWAP_REPLY_ID,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            }
        ]
    );
}

#[test]
fn execute_swap_operation() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        oroswap_factory: String::from("oroswapfactory"),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    deps.querier
        .with_oroswap_pairs(&[(&"uusdasset".to_string(), &String::from("pair"), PairType::Xyk {})]);
    deps.querier.with_balance(&[(
        &String::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            amount: Uint128::new(1000000u128),
            denom: "uusd".to_string(),
        }],
    )]);

    deps.querier
        .with_oroswap_pairs(&[(&"assetuusd".to_string(), &String::from("pair"), PairType::Xyk {})]);
    deps.querier.with_token_balances(&[(
        &String::from("asset"),
        &[(
            &String::from(MOCK_CONTRACT_ADDR),
            &Uint128::new(1000000u128),
        )],
    )]);
    let msg = ExecuteMsg::ExecuteSwapOperation {
        operation: SwapOperation::OroSwap {
            offer_asset_info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset"),
            },
            ask_asset_info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            pair_type: PairType::Xyk {},
        },
        to: Some(String::from("addr0000")),
        max_spread: None,
        single: true,
    };
    let env = mock_env();
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg {
            msg: WasmMsg::Execute {
                contract_addr: String::from("asset"),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Send {
                    contract: String::from("pair"),
                    amount: Uint128::new(1000000u128),
                    msg: to_json_binary(&oroswap::pair::Cw20HookMsg::Swap {
                        ask_asset_info: Some(native_asset_info("uusd".to_string())),
                        belief_price: None,
                        max_spread: None,
                        to: Some(String::from("addr0000")),
                    })
                    .unwrap()
                })
                .unwrap()
            }
            .into(),
            id: 0,
            gas_limit: None,
            reply_on: ReplyOn::Never,
        }]
    );
}

#[test]
fn query_buy_with_routes() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        oroswap_factory: String::from("oroswapfactory"),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = QueryMsg::SimulateSwapOperations {
        offer_amount: Uint128::from(1000000u128),
        operations: vec![
            SwapOperation::OroSwap {
                offer_asset_info: AssetInfo::NativeToken {
                    denom: "ukrw".to_string(),
                },
                ask_asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                pair_type: PairType::Xyk {},
            },
            SwapOperation::OroSwap {
                offer_asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                ask_asset_info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                pair_type: PairType::Xyk {},
            },
        ],
    };
    deps.querier.with_oroswap_pairs(&[
        (&"ukrwasset0000".to_string(), &String::from("pair0000"), PairType::Xyk {}),
        (&"asset0000uluna".to_string(), &String::from("pair0001"), PairType::Xyk {}),
    ]);

    let res: SimulateSwapOperationsResponse =
        from_json(&query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();
    assert_eq!(
        res,
        SimulateSwapOperationsResponse {
            amount: Uint128::from(1000000u128),
            router_stages: vec![
                RouterStage {
                    stage: 0,
                    pair_address: "pair0000".to_string(),
                    offer_asset: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    ask_asset: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    return_amount: Uint128::from(1000000u128),
                    commission_amount: Uint128::zero(),
                    spread_amount: Uint128::zero(),
                },
                RouterStage {
                    stage: 1,
                    pair_address: "pair0001".to_string(),
                    offer_asset: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    ask_asset: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                    return_amount: Uint128::from(1000000u128),
                    commission_amount: Uint128::zero(),
                    spread_amount: Uint128::zero(),
                }
            ]
        }
    );

    assert_eq!(
        res,
        SimulateSwapOperationsResponse {
            amount: Uint128::from(1000000u128),
            router_stages: vec![
                RouterStage {
                    stage: 0,
                    pair_address: "pair0000".to_string(),
                    offer_asset: AssetInfo::NativeToken {
                        denom: "ukrw".to_string(),
                    },
                    ask_asset: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    return_amount: Uint128::from(1000000u128),
                    commission_amount: Uint128::zero(),
                    spread_amount: Uint128::zero(),
                },
                RouterStage {
                    stage: 1,
                    pair_address: "pair0001".to_string(),
                    offer_asset: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    ask_asset: AssetInfo::NativeToken {
                        denom: "uluna".to_string(),
                    },
                    return_amount: Uint128::from(1000000u128),
                    commission_amount: Uint128::zero(),
                    spread_amount: Uint128::zero(),
                }
            ]
        }
    );

    let msg = QueryMsg::SimulateSwapOperations {
        offer_amount: Uint128::from(1000000u128),
        operations: vec![SwapOperation::NativeSwap {
            offer_denom: "ukrw".to_string(),
            ask_denom: "test".to_string(),
        }],
    };
    let err = query(deps.as_ref(), env.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::NativeSwapNotSupported {});
}

#[test]
fn assert_maximum_receive_swap_operations() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        oroswap_factory: String::from("oroswapfactory"),
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::ExecuteSwapOperations {
        operations: vec![
            SwapOperation::NativeSwap {
                offer_denom: "uusd".to_string(),
                ask_denom: "ukrw".to_string(),
            };
            MAX_SWAP_OPERATIONS + 1
        ],
        minimum_receive: None,
        to: None,
        max_spread: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(res, ContractError::SwapLimitExceeded {});
}
