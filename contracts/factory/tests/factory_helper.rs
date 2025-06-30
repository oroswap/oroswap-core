#![cfg(not(tarpaulin_include))]

use anyhow::Result as AnyResult;
use oroswap::asset::AssetInfo;
use oroswap::factory::{PairConfig, PairType, TrackerConfig};
use oroswap_test::cw_multi_test::{AppBuilder, AppResponse, ContractWrapper, Executor, BankSudo};
use oroswap_test::modules::stargate::StargateApp as TestApp;
use oroswap_test::modules::stargate::MockStargate;

use cosmwasm_std::{Addr, Binary, StdResult, Uint128, Coin, coins};
use cw20::MinterResponse;
use oroswap_factory::contract;

pub struct FactoryHelper {
    pub owner: Addr,
    pub oro_token: Addr,
    pub factory: Addr,
    pub cw20_token_code_id: u64,
}

impl FactoryHelper {
    pub fn init(router: &mut TestApp, owner: &Addr) -> Self {
        let oro_token_contract = Box::new(ContractWrapper::new_with_empty(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query,
        ));

        let cw20_token_code_id = router.store_code(oro_token_contract);

        let msg = oroswap::token::InstantiateMsg {
            name: String::from("Oro token"),
            symbol: String::from("ORO"),
            decimals: 6,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: owner.to_string(),
                cap: None,
            }),
            marketing: None,
        };

        let oro_token = router
            .instantiate_contract(
                cw20_token_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("ORO"),
                None,
            )
            .unwrap();

        let pair_contract = Box::new(
            ContractWrapper::new_with_empty(
                oroswap_pair::contract::execute,
                oroswap_pair::contract::instantiate,
                oroswap_pair::contract::query,
            )
            .with_reply_empty(oroswap_pair::contract::reply),
        );

        let pair_code_id = router.store_code(pair_contract);

        let factory_contract = Box::new(
            ContractWrapper::new_with_empty(
                contract::execute,
                contract::instantiate,
                contract::query,
            )
            .with_reply_empty(contract::reply),
        );

        let factory_code_id = router.store_code(factory_contract);

        let msg = oroswap::factory::InstantiateMsg {
            pair_configs: vec![
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Xyk {},
                    total_fee_bps: 100,
                    maker_fee_bps: 10,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                    pool_creation_fee: Uint128::new(1000),
                },
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Custom("transmuter".to_string()),
                    total_fee_bps: 0,
                    maker_fee_bps: 0,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: true,
                    pool_creation_fee: Uint128::new(1000),
                },
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Custom("concentrated".to_string()),
                    total_fee_bps: 100,
                    maker_fee_bps: 10,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                    pool_creation_fee: Uint128::new(1000),
                },
            ],
            token_code_id: cw20_token_code_id,
            fee_address: Some(owner.to_string()),
            generator_address: None,
            owner: owner.to_string(),
            whitelist_code_id: 0,
            coin_registry_address: "coin_registry".to_string(),
            tracker_config: None,
        };

        let factory = router
            .instantiate_contract(
                factory_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("ORO"),
                None,
            )
            .unwrap();

        Self {
            owner: owner.clone(),
            oro_token,
            factory,
            cw20_token_code_id,
        }
    }

    pub fn update_config(
        &mut self,
        router: &mut TestApp,
        sender: &Addr,
        token_code_id: Option<u64>,
        fee_address: Option<String>,
        generator_address: Option<String>,
        whitelist_code_id: Option<u64>,
        coin_registry_address: Option<String>,
    ) -> AnyResult<AppResponse> {
        let msg = oroswap::factory::ExecuteMsg::UpdateConfig {
            token_code_id,
            fee_address,
            generator_address,
            whitelist_code_id,
            coin_registry_address,
        };

        router.execute_contract(sender.clone(), self.factory.clone(), &msg, &[])
    }

    pub fn create_pair(
        &mut self,
        router: &mut TestApp,
        sender: &Addr,
        pair_type: PairType,
        tokens: [&Addr; 2],
        init_params: Option<Binary>,
    ) -> AnyResult<AppResponse> {
        let asset_infos = vec![
            AssetInfo::Token {
                contract_addr: tokens[0].clone(),
            },
            AssetInfo::Token {
                contract_addr: tokens[1].clone(),
            },
        ];

        let msg = oroswap::factory::ExecuteMsg::CreatePair {
            pair_type,
            asset_infos,
            init_params,
        };

        router.execute_contract(sender.clone(), self.factory.clone(), &msg, &[Coin {
            denom: "uzig".to_string(),
            amount: Uint128::new(1000),
        }])
    }

    pub fn update_tracker_config(
        &mut self,
        router: &mut TestApp,
        sender: &Addr,
        tracker_code_id: u64,
        token_factory_addr: Option<String>,
    ) -> AnyResult<AppResponse> {
        let msg = oroswap::factory::ExecuteMsg::UpdateTrackerConfig {
            tracker_code_id,
            token_factory_addr,
        };

        router.execute_contract(sender.clone(), self.factory.clone(), &msg, &[])
    }

    pub fn query_tracker_config(&mut self, router: &mut TestApp) -> StdResult<TrackerConfig> {
        let msg = oroswap::factory::QueryMsg::TrackerConfig {};
        router
            .wrap()
            .query_wasm_smart::<TrackerConfig>(self.factory.clone(), &msg)
    }
}

pub fn instantiate_token(
    app: &mut TestApp,
    token_code_id: u64,
    owner: &Addr,
    token_name: &str,
    decimals: Option<u8>,
) -> Addr {
    let init_msg = oroswap::token::InstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: decimals.unwrap_or(6),
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    app.instantiate_contract(
        token_code_id,
        owner.clone(),
        &init_msg,
        &[],
        token_name,
        None,
    )
    .unwrap()
}

fn mock_app() -> TestApp {
    let mut app = AppBuilder::new_custom()
        .with_stargate(MockStargate::default())
        .build(|_, _, _| {});

    // Add initial balances for test accounts
    app.sudo(
        BankSudo::Mint {
            to_address: "owner".to_string(),
            amount: coins(10000, "uzig"),
        }
        .into(),
    )
    .unwrap();

    app.sudo(
        BankSudo::Mint {
            to_address: "random_stranger".to_string(),
            amount: coins(10000, "uzig"),
        }
        .into(),
    )
    .unwrap();

    app
}
