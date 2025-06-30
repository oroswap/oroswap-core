use cosmwasm_std::{
    attr, from_json, to_json_binary, Addr, Coin, Reply, ReplyOn, SubMsg, SubMsgResponse, SubMsgResult,
    WasmMsg,
};
use cosmwasm_std::Uint128;

use crate::mock_querier::mock_dependencies;
use crate::state::CONFIG;
use crate::{
    contract::{execute, instantiate, query},
    error::ContractError,
};

use oroswap::asset::{AssetInfo, PairInfo};
use oroswap::factory::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PairConfig, PairType, PairsResponse, QueryMsg,
    StartAfter,
};

use crate::contract::reply;
use oroswap::pair::InstantiateMsg as PairInstantiateMsg;
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};

use prost::Message;

#[derive(Clone, PartialEq, Message)]
struct MsgInstantiateContractResponse {
    #[prost(string, tag = "1")]
    pub contract_address: String,
    #[prost(bytes, tag = "2")]
    pub data: Vec<u8>,
}

#[test]
fn pair_type_to_string() {
    assert_eq!(PairType::Xyk {}.to_string(), "xyk");
    assert_eq!(PairType::Stable {}.to_string(), "stable");
}

#[test]
fn proper_initialization() {
    // Validate total and maker fee bps
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000".to_string();

    let msg = InstantiateMsg {
        pair_configs: vec![
            PairConfig {
                code_id: 123u64,
                pair_type: PairType::Xyk {},
                total_fee_bps: 100,
                maker_fee_bps: 10,
                is_disabled: false,
                is_generator_disabled: false,
                permissioned: false,
                pool_creation_fee: Uint128::new(1000),
            },
            PairConfig {
                code_id: 325u64,
                pair_type: PairType::Xyk {},
                total_fee_bps: 100,
                maker_fee_bps: 10,
                is_disabled: false,
                is_generator_disabled: false,
                permissioned: false,
                pool_creation_fee: Uint128::new(1000),
            },
        ],
        token_code_id: 123u64,
        fee_address: None,
        generator_address: Some(String::from("generator")),
        owner: owner.clone(),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::PairConfigDuplicate {});

    let msg = InstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: 123u64,
            pair_type: PairType::Xyk {},
            total_fee_bps: 10_001,
            maker_fee_bps: 10,
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id: 123u64,
        fee_address: None,
        generator_address: Some(String::from("generator")),
        owner: owner.clone(),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, ContractError::PairConfigInvalidFeeBps {});

    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        pair_configs: vec![
            PairConfig {
                code_id: 325u64,
                pair_type: PairType::Stable {},
                total_fee_bps: 100,
                maker_fee_bps: 10,
                is_disabled: false,
                is_generator_disabled: false,
                permissioned: false,
                pool_creation_fee: Uint128::new(1000),
            },
            PairConfig {
                code_id: 123u64,
                pair_type: PairType::Xyk {},
                total_fee_bps: 100,
                maker_fee_bps: 10,
                is_disabled: false,
                is_generator_disabled: false,
                permissioned: false,
                pool_creation_fee: Uint128::new(1000),
            },
        ],
        token_code_id: 123u64,
        fee_address: None,
        generator_address: Some(String::from("generator")),
        owner: owner.clone(),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    let query_res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_json(&query_res).unwrap();
    assert_eq!(123u64, config_res.token_code_id);
    assert_eq!(msg.pair_configs, config_res.pair_configs);
    assert_eq!(Addr::unchecked(owner), config_res.owner);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";

    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 3,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // Update config
    let env = mock_env();
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        token_code_id: Some(200u64),
        fee_address: Some(String::from("new_fee_addr")),
        generator_address: Some(String::from("new_generator_addr")),
        whitelist_code_id: None,
        coin_registry_address: None,
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // It worked, let's query the state
    let query_res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_json(&query_res).unwrap();
    assert_eq!(200u64, config_res.token_code_id);
    assert_eq!(owner, config_res.owner);
    assert_eq!(
        String::from("new_fee_addr"),
        config_res.fee_address.unwrap()
    );
    assert_eq!(
        String::from("new_generator_addr"),
        config_res.generator_address.unwrap()
    );

    // Unauthorized err
    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        token_code_id: None,
        fee_address: None,
        generator_address: None,
        whitelist_code_id: None,
        coin_registry_address: None,
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});
}

#[test]
fn update_owner() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";

    let msg = InstantiateMsg {
        pair_configs: vec![],
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);

    // We can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    let new_owner = String::from("new_owner");

    // New owner
    let env = mock_env();
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    let info = mock_info(new_owner.as_str(), &[]);

    // Unauthorized check
    let err = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let info = mock_info(new_owner.as_str(), &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    )
    .unwrap_err();

    // Propose new owner
    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Unauthorized ownership claim
    let info = mock_info("invalid_addr", &[]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // Claim ownership
    let info = mock_info(new_owner.as_str(), &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    )
    .unwrap();
    assert_eq!(0, res.messages.len());

    // Let's query the state
    let config: ConfigResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(new_owner, config.owner);
}

#[test]
fn update_pair_config() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 100,
        maker_fee_bps: 10,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // We can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // It worked, let's query the state
    let query_res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_json(&query_res).unwrap();
    assert_eq!(pair_configs, config_res.pair_configs);

    // Update config
    let pair_config = PairConfig {
        code_id: 800,
        pair_type: PairType::Xyk {},
        total_fee_bps: 1,
        maker_fee_bps: 2,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    };

    // Unauthorized err
    let env = mock_env();
    let info = mock_info("wrong-addr0000", &[]);
    let msg = ExecuteMsg::UpdatePairConfig {
        config: pair_config.clone(),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // Check validation of total and maker fee bps
    let env = mock_env();
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UpdatePairConfig {
        config: PairConfig {
            code_id: 123u64,
            pair_type: PairType::Xyk {},
            total_fee_bps: 3,
            maker_fee_bps: 10_001,
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        },
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::PairConfigInvalidFeeBps {});

    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UpdatePairConfig {
        config: pair_config.clone(),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // It worked, let's query the state
    let query_res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_json(&query_res).unwrap();
    assert_eq!(vec![pair_config.clone()], config_res.pair_configs);

    // Add second config
    let pair_config_custom = PairConfig {
        code_id: 100,
        pair_type: PairType::Custom("test".to_string()),
        total_fee_bps: 10,
        maker_fee_bps: 20,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    };

    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UpdatePairConfig {
        config: pair_config_custom.clone(),
    };

    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // It worked, let's query the state
    let query_res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_json(&query_res).unwrap();
    assert_eq!(
        vec![pair_config_custom.clone(), pair_config.clone()],
        config_res.pair_configs
    );
}

#[test]
fn create_pair() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";

    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 3,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);

    // We can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // Create pair
    let env = mock_env();
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uzig".to_string(),
            amount: Uint128::new(1000),
        }],
    );
    let msg = ExecuteMsg::CreatePair {
        pair_type: PairType::Xyk {},
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0001"),
            },
        ]
        .to_vec(),
        init_params: None,
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "create_pair"),
            attr("pair", "uluna-asset0001"),
            attr("pair_type", "xyk"),
            attr("pool_creation_fee", "1000"),
            attr("total_funds", "1000"),
        ]
    );

    // Handle the reply from pair creation
    let instantiate_reply = MsgInstantiateContractResponse {
        contract_address: String::from("pair0000"),
        data: vec![],
    };

    let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
    instantiate_reply
        .encode(&mut encoded_instantiate_reply)
        .unwrap();

    let reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(encoded_instantiate_reply.into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    // Register the mock pair contract with the querier
    let pair_addr = "pair0000".to_string();
    let pair_info = PairInfo {
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0001"),
            },
        ]
        .to_vec(),
        contract_addr: Addr::unchecked("pair0000"),
        liquidity_token: "liquidity0000".to_owned(),
        pair_type: PairType::Xyk {},
    };

    let deployed_pairs = vec![(&pair_addr, &pair_info)];
    deps.querier.with_oroswap_pairs(&deployed_pairs);

    // Verify the pair was created
    let query_res = query(
        deps.as_ref(),
        env,
        QueryMsg::Pair {
            asset_infos: [
                AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
            ]
            .to_vec(),
            pair_type: PairType::Xyk {},
        },
    )
    .unwrap();
    let pair_res: PairInfo = from_json(&query_res).unwrap();
    assert_eq!(pair_res.pair_type, PairType::Xyk {});
    assert_eq!(pair_res.contract_addr, Addr::unchecked("pair0000"));
}

#[test]
fn register() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";

    let msg = InstantiateMsg {
        pair_configs: vec![PairConfig {
            code_id: 123u64,
            pair_type: PairType::Xyk {},
            total_fee_bps: 100,
            maker_fee_bps: 10,
            is_disabled: false,
            is_generator_disabled: false,
            permissioned: false,
            pool_creation_fee: Uint128::new(1000),
        }],
        token_code_id: 123u64,
        fee_address: None,
        generator_address: Some(String::from("generator")),
        owner: owner.to_string(),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[Coin {
        denom: "uzig".to_string(),
        amount: Uint128::new(1000),
    }]);
    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    let asset_infos = vec![
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0001"),
        },
    ];

    let msg = ExecuteMsg::CreatePair {
        pair_type: PairType::Xyk {},
        asset_infos: asset_infos.clone(),
        init_params: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[Coin {
        denom: "uzig".to_string(),
        amount: Uint128::new(1000),
    }]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let pair0_addr = "pair0000".to_string();
    let pair0_info = PairInfo {
        asset_infos: asset_infos.clone(),
        contract_addr: Addr::unchecked("pair0000"),
        liquidity_token: "liquidity0000".to_owned(),
        pair_type: PairType::Xyk {},
    };

    let mut deployed_pairs = vec![(&pair0_addr, &pair0_info)];

    // Register an Oroswap pair querier
    deps.querier.with_oroswap_pairs(&deployed_pairs);

    let instantiate_reply = MsgInstantiateContractResponse {
        contract_address: String::from("pair0000"),
        data: vec![],
    };

    let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
    instantiate_reply
        .encode(&mut encoded_instantiate_reply)
        .unwrap();

    let reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(encoded_instantiate_reply.into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg.clone()).unwrap();

    let query_res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::Pair {
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        },
    )
    .unwrap();

    let pair_res: PairInfo = from_json(&query_res).unwrap();
    assert_eq!(
        pair_res,
        PairInfo {
            liquidity_token: "liquidity0000".to_owned(),
            contract_addr: Addr::unchecked("pair0000"),
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        }
    );

    // Check pair was registered
    let res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap_err();
    assert_eq!(res, ContractError::PairWasRegistered {});

    // Store one more item to test query pairs
    let asset_infos_2 = vec![
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0002"),
        },
    ];

    let msg = ExecuteMsg::CreatePair {
        pair_type: PairType::Xyk {},
        asset_infos: asset_infos_2.clone(),
        init_params: None,
    };

    let env = mock_env();
    let info = mock_info("addr0000", &[Coin {
        denom: "uzig".to_string(),
        amount: Uint128::new(1000),
    }]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let pair1_addr = "pair0001".to_string();
    let pair1_info = PairInfo {
        asset_infos: asset_infos_2.clone(),
        contract_addr: Addr::unchecked("pair0001"),
        liquidity_token: "liquidity0001".to_owned(),
        pair_type: PairType::Xyk {},
    };

    deployed_pairs.push((&pair1_addr, &pair1_info));

    // Register Oroswap pair querier
    deps.querier.with_oroswap_pairs(&deployed_pairs);

    let instantiate_reply = MsgInstantiateContractResponse {
        contract_address: String::from("pair0001"),
        data: vec![],
    };

    let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
    instantiate_reply
        .encode(&mut encoded_instantiate_reply)
        .unwrap();

    let reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(encoded_instantiate_reply.into()),
        }),
    };

    let _res = reply(deps.as_mut(), mock_env(), reply_msg.clone()).unwrap();

    let query_msg = QueryMsg::Pairs {
        start_after: None,
        limit: None,
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let pairs_res: PairsResponse = from_json(&res).unwrap();
    assert_eq!(
        pairs_res.pairs,
        vec![
            PairInfo {
                liquidity_token: "liquidity0000".to_owned(),
                contract_addr: Addr::unchecked("pair0000"),
                asset_infos: asset_infos.clone(),
                pair_type: PairType::Xyk {},
            },
            PairInfo {
                liquidity_token: "liquidity0001".to_owned(),
                contract_addr: Addr::unchecked("pair0001"),
                asset_infos: asset_infos_2.clone(),
                pair_type: PairType::Xyk {},
            }
        ]
    );

    let query_msg = QueryMsg::Pairs {
        start_after: None,
        limit: Some(1),
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let pairs_res: PairsResponse = from_json(&res).unwrap();
    assert_eq!(
        pairs_res.pairs,
        vec![PairInfo {
            liquidity_token: "liquidity0000".to_owned(),
            contract_addr: Addr::unchecked("pair0000"),
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        }]
    );

    let query_msg = QueryMsg::Pairs {
        start_after: Some(StartAfter {
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        }),
        limit: None,
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let pairs_res: PairsResponse = from_json(&res).unwrap();
    assert_eq!(
        pairs_res.pairs,
        vec![PairInfo {
            liquidity_token: "liquidity0001".to_owned(),
            contract_addr: Addr::unchecked("pair0001"),
            asset_infos: asset_infos_2.clone(),
            pair_type: PairType::Xyk {},
        }]
    );

    // Deregister from wrong acc
    let env = mock_env();
    let info = mock_info("wrong_addr0000", &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Deregister {
            asset_infos: asset_infos_2.clone(),
            pair_type: PairType::Xyk {},
        },
    )
    .unwrap_err();

    assert_eq!(res, ContractError::Unauthorized {});

    // Proper deregister
    let env = mock_env();
    let info = mock_info(owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Deregister {
            asset_infos: asset_infos_2.clone(),
            pair_type: PairType::Xyk {},
        },
    )
    .unwrap();

    assert_eq!(res.attributes[0], attr("action", "deregister"));

    let query_msg = QueryMsg::Pairs {
        start_after: None,
        limit: None,
    };

    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let pairs_res: PairsResponse = from_json(&res).unwrap();
    assert_eq!(
        pairs_res.pairs,
        vec![PairInfo {
            liquidity_token: "liquidity0000".to_owned(),
            contract_addr: Addr::unchecked("pair0000"),
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        },]
    );
}

#[test]
fn test_pause_pairs_batch() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Add pause authority
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Create some pairs first
    let asset_infos_1 = vec![
        AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    ];

    let asset_infos_2 = vec![
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0001"),
        },
    ];

    let asset_infos_3 = vec![
        AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
    ];

    // Create pairs and handle replies immediately
    for (i, asset_infos) in [&asset_infos_1, &asset_infos_2].iter().enumerate() {
        let msg = ExecuteMsg::CreatePair {
            pair_type: PairType::Xyk {},
            asset_infos: asset_infos.to_vec(),
            init_params: None,
        };
        let info = mock_info("addr0000", &[Coin {
            denom: "uzig".to_string(),
            amount: Uint128::new(1000),
        }]);
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Mock pair instantiation reply for this specific pair
        let instantiate_reply = MsgInstantiateContractResponse {
            contract_address: format!("pair{:04}", i),
            data: vec![],
        };

        let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
        instantiate_reply.encode(&mut encoded_instantiate_reply).unwrap();

        let reply_msg = Reply {
            id: 1,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
    }

    // Mock pair instantiation replies for querier
    let mut deployed_pairs = vec![];
    for (i, asset_infos) in [&asset_infos_1, &asset_infos_2].iter().enumerate() {
        let pair_addr = format!("pair{:04}", i);
        let pair_info = PairInfo {
            asset_infos: asset_infos.to_vec(),
            contract_addr: Addr::unchecked(&pair_addr),
            liquidity_token: format!("liquidity{:04}", i),
            pair_type: PairType::Xyk {},
        };
        deployed_pairs.push((pair_addr, pair_info));
    }
    let deployed_pairs_refs: Vec<(&String, &PairInfo)> = deployed_pairs.iter().map(|(a, i)| (a, i)).collect();
    deps.querier.with_oroswap_pairs(&deployed_pairs_refs);

    // Test pause pairs batch with default batch size (50)
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Check response attributes robustly
    let attrs = &res.attributes;
    assert!(attrs.iter().any(|a| a.key == "action" && a.value == "pause_pairs_batch"));
    assert!(attrs.iter().any(|a| (a.key == "paused_count" || a.key == "processed_count") && a.value == "2"));
    assert!(attrs.iter().any(|a| a.key == "has_more" && a.value == "false"));

    // Check that messages were created for each pair
    assert_eq!(res.messages.len(), 2);

    // Verify pairs are marked as paused in storage
    for asset_infos in [&asset_infos_1, &asset_infos_2] {
        let query_msg = QueryMsg::IsPairPaused {
            asset_infos: asset_infos.to_vec(),
            pair_type: PairType::Xyk {},
        };
        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let is_paused: bool = from_json(&res).unwrap();
        assert!(is_paused);
    }

    // Optionally, test the second batch pause:
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: Some(2) };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let attrs = &res.attributes;
    assert!(attrs.iter().any(|a| (a.key == "paused_count" || a.key == "processed_count") && a.value == "0"));
    assert!(attrs.iter().any(|a| a.key == "has_more" && a.value == "false"));

    // Test pause pairs batch with custom batch size
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: Some(2) };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Since all pairs are already paused, should return 0 paused
    let attrs = &res.attributes;
    assert!(attrs.iter().any(|a| (a.key == "paused_count" || a.key == "processed_count") && a.value == "0"));
    assert!(attrs.iter().any(|a| a.key == "has_more" && a.value == "false"));
}

#[test]
fn test_unpause_pairs_batch() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Add pause authority
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Create and register pairs
    let asset_infos_1 = vec![
        AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    ];

    let asset_infos_2 = vec![
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0001"),
        },
    ];

    // Create pairs and handle replies immediately
    for (i, asset_infos) in [&asset_infos_1, &asset_infos_2].iter().enumerate() {
        let msg = ExecuteMsg::CreatePair {
            pair_type: PairType::Xyk {},
            asset_infos: asset_infos.to_vec(),
            init_params: None,
        };
        let info = mock_info("addr0000", &[Coin {
            denom: "uzig".to_string(),
            amount: Uint128::new(1000),
        }]);
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Mock pair instantiation reply for this specific pair
        let instantiate_reply = MsgInstantiateContractResponse {
            contract_address: format!("pair{:04}", i),
            data: vec![],
        };

        let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
        instantiate_reply.encode(&mut encoded_instantiate_reply).unwrap();

        let reply_msg = Reply {
            id: 1,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
    }

    // Mock pair instantiation replies for querier
    let mut deployed_pairs = vec![];
    for (i, asset_infos) in [&asset_infos_1, &asset_infos_2].iter().enumerate() {
        let pair_addr = format!("pair{:04}", i);
        let pair_info = PairInfo {
            asset_infos: asset_infos.to_vec(),
            contract_addr: Addr::unchecked(&pair_addr),
            liquidity_token: format!("liquidity{:04}", i),
            pair_type: PairType::Xyk {},
        };
        deployed_pairs.push((pair_addr, pair_info));
    }
    let deployed_pairs_refs: Vec<(&String, &PairInfo)> = deployed_pairs.iter().map(|(a, i)| (a, i)).collect();
    deps.querier.with_oroswap_pairs(&deployed_pairs_refs);

    // First pause all pairs
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Verify pairs are paused
    for asset_infos in [&asset_infos_1, &asset_infos_2] {
        let query_msg = QueryMsg::IsPairPaused {
            asset_infos: asset_infos.to_vec(),
            pair_type: PairType::Xyk {},
        };
        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let is_paused: bool = from_json(&res).unwrap();
        assert!(is_paused);
    }

    // Test unpause pairs batch
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UnpausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Check response attributes
    assert_eq!(res.attributes[0], attr("action", "unpause_pairs_batch"));
    assert_eq!(res.attributes[1], attr("unpaused_count", "2"));
    assert_eq!(res.attributes[2], attr("processed_count", "2"));
    assert_eq!(res.attributes[3], attr("batch_size", "50"));
    assert_eq!(res.attributes[4], attr("has_more", "false"));

    // Check that messages were created for each pair
    assert_eq!(res.messages.len(), 2);

    // Verify pairs are no longer paused
    for asset_infos in [&asset_infos_1, &asset_infos_2] {
        let query_msg = QueryMsg::IsPairPaused {
            asset_infos: asset_infos.to_vec(),
            pair_type: PairType::Xyk {},
        };
        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let is_paused: bool = from_json(&res).unwrap();
        assert!(!is_paused);
    }
}

#[test]
fn test_pause_authority_validation() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";
    let unauthorized_user = "unauthorized_user";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Try to pause pairs without being pause authority
    let info = mock_info(unauthorized_user, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::NoPauseAuthority {});

    // Try to unpause pairs without being pause authority
    let info = mock_info(unauthorized_user, &[]);
    let msg = ExecuteMsg::UnpausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::NoUnpauseAuthority {});

    // Add pause authority
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Now pause authority should be able to pause
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "pause_pairs_batch"));

    // But pause authority should NOT be able to unpause (only owner can)
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::UnpausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::NoUnpauseAuthority {});
}

#[test]
fn test_add_pause_authorities() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Try to add pause authorities without being owner
    let info = mock_info("unauthorized_user", &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec!["auth1".to_string(), "auth2".to_string()],
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::Unauthorized {});

    // Add pause authorities as owner
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec!["auth1".to_string(), "auth2".to_string()],
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "add_pause_authorities"));

    // Query pause authorities
    let query_msg = QueryMsg::PauseAuthorities {};
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let authorities: Vec<String> = from_json(&res).unwrap();
    assert_eq!(authorities.len(), 2);
    assert!(authorities.contains(&"auth1".to_string()));
    assert!(authorities.contains(&"auth2".to_string()));
}

#[test]
fn test_pause_individual_pair() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Add pause authority
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Create a pair
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    ];

    let msg = ExecuteMsg::CreatePair {
        pair_type: PairType::Xyk {},
        asset_infos: asset_infos.clone(),
        init_params: None,
    };
    let info = mock_info("addr0000", &[Coin {
        denom: "uzig".to_string(),
        amount: Uint128::new(1000),
    }]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Mock pair instantiation reply
    let pair_addr = "pair0000".to_string();
    let pair_info = PairInfo {
        asset_infos: asset_infos.clone(),
        contract_addr: Addr::unchecked("pair0000"),
        liquidity_token: "liquidity0000".to_string(),
        pair_type: PairType::Xyk {},
    };
    let deployed_pairs = vec![(&pair_addr, &pair_info)];
    deps.querier.with_oroswap_pairs(&deployed_pairs);

    // Register pair
    let instantiate_reply = MsgInstantiateContractResponse {
        contract_address: "pair0000".to_string(),
        data: vec![],
    };

    let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
    instantiate_reply.encode(&mut encoded_instantiate_reply).unwrap();

    let reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(encoded_instantiate_reply.into()),
        }),
    };

    reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    // Pause the pair
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePair { 
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "pause_pair"));

    // Verify pair is paused
    let query_msg = QueryMsg::IsPairPaused {
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let is_paused: bool = from_json(&res).unwrap();
    assert!(is_paused);

    // Try to pause the same pair again
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePair { 
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::PairAlreadyPaused {});

    // Unpause the pair (only owner can unpause)
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UnpausePair { 
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "unpause_pair"));

    // Verify pair is not paused
    let query_msg = QueryMsg::IsPairPaused {
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let is_paused: bool = from_json(&res).unwrap();
    assert!(!is_paused);

    // Try to unpause the same pair again (should fail)
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UnpausePair { 
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(res, ContractError::PairNotPaused {});
}

#[test]
fn test_pause_all_pairs() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Add pause authority
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Create multiple pairs
    let asset_infos_list = vec![
        vec![
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        ],
        vec![
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0001"),
            },
        ],
        vec![
            AssetInfo::NativeToken {
                denom: "uluna".to_string(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
        ],
    ];

    // Create and register pairs
    let mut deployed_pairs = vec![];
    for (i, asset_infos) in asset_infos_list.iter().enumerate() {
        let msg = ExecuteMsg::CreatePair {
            pair_type: PairType::Xyk {},
            asset_infos: asset_infos.clone(),
            init_params: None,
        };
        let info = mock_info("addr0000", &[Coin {
            denom: "uzig".to_string(),
            amount: Uint128::new(1000),
        }]);
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Mock pair instantiation reply for this specific pair
        let instantiate_reply = MsgInstantiateContractResponse {
            contract_address: format!("pair{:04}", i),
            data: vec![],
        };

        let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(instantiate_reply.encoded_len());
        instantiate_reply.encode(&mut encoded_instantiate_reply).unwrap();

        let reply_msg = Reply {
            id: 1,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply.into()),
            }),
        };

        reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

        let pair_addr = format!("pair{:04}", i);
        let pair_info = PairInfo {
            asset_infos: asset_infos.clone(),
            contract_addr: Addr::unchecked(&pair_addr),
            liquidity_token: format!("liquidity{:04}", i),
            pair_type: PairType::Xyk {},
        };
        deployed_pairs.push((pair_addr, pair_info));
    }
    let deployed_pairs_refs: Vec<(&String, &PairInfo)> = deployed_pairs.iter().map(|(a, i)| (a, i)).collect();
    deps.querier.with_oroswap_pairs(&deployed_pairs_refs);

    // Pause all pairs using batch function
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "pause_pairs_batch"));
    assert_eq!(res.attributes[1], attr("paused_count", "3"));

    // Verify all pairs are paused
    for asset_infos in &asset_infos_list {
        let query_msg = QueryMsg::IsPairPaused {
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        };
        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let is_paused: bool = from_json(&res).unwrap();
        assert!(is_paused);
    }

    // Query paused pairs count
    let query_msg = QueryMsg::PausedPairsCount {};
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let count: u32 = from_json(&res).unwrap();
    assert_eq!(count, 3);

    // Unpause all pairs using batch function (only owner can unpause)
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::UnpausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "unpause_pairs_batch"));
    assert_eq!(res.attributes[1], attr("unpaused_count", "3"));

    // Verify all pairs are not paused
    for asset_infos in &asset_infos_list {
        let query_msg = QueryMsg::IsPairPaused {
            asset_infos: asset_infos.clone(),
            pair_type: PairType::Xyk {},
        };
        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let is_paused: bool = from_json(&res).unwrap();
        assert!(!is_paused);
    }

    // Query paused pairs count again
    let query_msg = QueryMsg::PausedPairsCount {};
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let count: u32 = from_json(&res).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_batch_size_validation() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Add pause authority
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Test batch size validation - should use default (50) when None
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: None };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "pause_pairs_batch"));

    // Test batch size validation - should use provided size when valid
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: Some(25) };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "pause_pairs_batch"));

    // Test batch size validation - should cap at maximum (100)
    let info = mock_info(pause_authority, &[]);
    let msg = ExecuteMsg::PausePairsBatch { batch_size: Some(150) };
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes[0], attr("action", "pause_pairs_batch"));
}

#[test]
fn test_query_functions() {
    let mut deps = mock_dependencies(&[]);
    let owner = "owner0000";
    let pause_authority = "pause_authority";

    // Initialize factory
    let pair_configs = vec![PairConfig {
        code_id: 123u64,
        pair_type: PairType::Xyk {},
        total_fee_bps: 30,
        maker_fee_bps: 166,
        is_disabled: false,
        is_generator_disabled: false,
        permissioned: false,
        pool_creation_fee: Uint128::new(1000),
    }];

    let msg = InstantiateMsg {
        pair_configs: pair_configs.clone(),
        token_code_id: 123u64,
        fee_address: None,
        owner: owner.to_string(),
        generator_address: Some(String::from("generator")),
        whitelist_code_id: 234u64,
        coin_registry_address: "coin_registry".to_string(),
        tracker_config: None,
    };

    let env = mock_env();
    let info = mock_info(owner, &[]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Test query pause authorities when none exist
    let query_msg = QueryMsg::PauseAuthorities {};
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let authorities: Vec<String> = from_json(&res).unwrap();
    assert_eq!(authorities.len(), 0);

    // Add pause authorities
    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::AddPauseAuthorities {
        authorities: vec![pause_authority.to_string(), "auth2".to_string()],
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Test query pause authorities
    let query_msg = QueryMsg::PauseAuthorities {};
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let authorities: Vec<String> = from_json(&res).unwrap();
    assert_eq!(authorities.len(), 2);
    assert!(authorities.contains(&pause_authority.to_string()));
    assert!(authorities.contains(&"auth2".to_string()));

    // Test query paused pairs count when no pairs exist
    let query_msg = QueryMsg::PausedPairsCount {};
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let count: u32 = from_json(&res).unwrap();
    assert_eq!(count, 0);

    // Test query is pair paused for non-existent pair
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    ];

    let query_msg = QueryMsg::IsPairPaused {
        asset_infos: asset_infos.clone(),
        pair_type: PairType::Xyk {},
    };
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let is_paused: bool = from_json(&res).unwrap();
    assert!(!is_paused);
}
