use cosmwasm_std::{coins, Addr, Decimal, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};
use oroswap_core::asset::{Asset, AssetInfo};
use oroswap_core::factory::PairType;

use oroswap_pool_initializer::contract::{execute, instantiate, query};
use oroswap_pool_initializer::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, ProvideLiquidityParams, QueryMsg,
};

#[test]
fn test_cw20_token_message_structure() {
    // Test that CW-20 token messages can be created without errors
    let msg = ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type: PairType::Xyk {},
        asset_infos: vec![
            AssetInfo::Token { contract_addr: Addr::unchecked("token1") },
            AssetInfo::Token { contract_addr: Addr::unchecked("token2") },
        ],
        init_params: None,
        liquidity: ProvideLiquidityParams {
            assets: vec![
                Asset {
                    info: AssetInfo::Token { contract_addr: Addr::unchecked("token1") },
                    amount: Uint128::new(100000), // 100 tokens
                },
                Asset {
                    info: AssetInfo::Token { contract_addr: Addr::unchecked("token2") },
                    amount: Uint128::new(200000), // 200 tokens
                },
            ],
            slippage_tolerance: Some(Decimal::percent(1)),
            auto_stake: Some(false),
            receiver: None,
            min_lp_to_receive: None,
        },
    };

    // This should not panic
    let _ = cosmwasm_std::to_json_string(&msg).unwrap();
    
    println!("✅ CW-20 token message structure test passed!");
    println!("📝 Message can be serialized successfully");
    println!("🎯 CW-20 tokens are properly handled in the message structure");
}

#[test]
fn test_mixed_token_message_structure() {
    // Test mixed native and CW-20 token messages
    let msg = ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type: PairType::Xyk {},
        asset_infos: vec![
            AssetInfo::NativeToken { denom: "uzig".to_string() },
            AssetInfo::Token { contract_addr: Addr::unchecked("token1") },
        ],
        init_params: None,
        liquidity: ProvideLiquidityParams {
            assets: vec![
                Asset {
                    info: AssetInfo::NativeToken { denom: "uzig".to_string() },
                    amount: Uint128::new(1000000), // 1 ZIG
                },
                Asset {
                    info: AssetInfo::Token { contract_addr: Addr::unchecked("token1") },
                    amount: Uint128::new(100000), // 100 tokens
                },
            ],
            slippage_tolerance: Some(Decimal::percent(1)),
            auto_stake: Some(false),
            receiver: None,
            min_lp_to_receive: None,
        },
    };

    // This should not panic
    let _ = cosmwasm_std::to_json_string(&msg).unwrap();
    
    println!("✅ Mixed token message structure test passed!");
    println!("📝 Mixed native/CW-20 message can be serialized successfully");
    println!("🎯 Both token types are properly handled in the message structure");
}

#[test]
fn test_message_structure() {
    let msg = ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type: PairType::Xyk {},
        asset_infos: vec![
            AssetInfo::NativeToken { denom: "uzig".to_string() },
            AssetInfo::NativeToken { denom: "uatom".to_string() },
        ],
        init_params: None,
        liquidity: ProvideLiquidityParams {
            assets: vec![
                Asset {
                    info: AssetInfo::NativeToken { denom: "uzig".to_string() },
                    amount: Uint128::new(1000000),
                },
                Asset {
                    info: AssetInfo::NativeToken { denom: "uatom".to_string() },
                    amount: Uint128::new(1000000),
                },
            ],
            slippage_tolerance: Some(Decimal::percent(1)),
            auto_stake: Some(false),
            receiver: None,
            min_lp_to_receive: None,
        },
    };

    // This should not panic
    let _ = cosmwasm_std::to_json_string(&msg).unwrap();
}

#[test]
fn test_instantiate() {
    let mut app = App::default();

    let code_id = app.store_code(Box::new(ContractWrapper::new(
        execute, instantiate, query,
    )));

    let msg = InstantiateMsg {
        factory_addr: "factory_addr".to_string(),
        pair_creation_fee: Uint128::new(101000000),
    };

    let contract_addr = app
        .instantiate_contract(code_id, Addr::unchecked("admin"), &msg, &[], "test", None)
        .unwrap();

    let config: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config.factory_addr, "factory_addr");
    assert_eq!(config.pair_creation_fee, Uint128::new(101000000));
}
