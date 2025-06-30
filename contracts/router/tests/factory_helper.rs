#![cfg(not(tarpaulin_include))]

use anyhow::Result as AnyResult;
use cosmwasm_std::{coin, coins, Addr, Binary, Uint128};
use cw20::MinterResponse;

use oroswap::asset::{AssetInfo, PairInfo};
use oroswap::factory::{PairConfig, PairType, ExecuteMsg as FactoryExecuteMsg, QueryMsg as FactoryQueryMsg};
use oroswap_test::cw_multi_test::{AppResponse, ContractWrapper, Executor, AppBuilder};
use oroswap_test::modules::stargate::{StargateApp as App, MockStargate};

pub struct FactoryHelper {
    pub owner: Addr,
    pub factory: Addr,
    pub coin_registry: Addr,
    pub cw20_token_code_id: u64,
}

impl FactoryHelper {
    pub fn init(router: &mut App, owner: &Addr) -> Self {
        let oro_token_contract = Box::new(ContractWrapper::new_with_empty(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query,
        ));

        let cw20_token_code_id = router.store_code(oro_token_contract);

        let pair_contract = Box::new(
            ContractWrapper::new_with_empty(
                oroswap_pair::contract::execute,
                oroswap_pair::contract::instantiate,
                oroswap_pair::contract::query,
            )
            .with_reply_empty(oroswap_pair::contract::reply),
        );

        let pair_code_id = router.store_code(pair_contract);

        let pcl_contract = Box::new(
            ContractWrapper::new_with_empty(
                oroswap_pair_concentrated::contract::execute,
                oroswap_pair_concentrated::contract::instantiate,
                oroswap_pair_concentrated::queries::query,
            )
            .with_reply_empty(oroswap_pair_concentrated::contract::reply),
        );
        let pcl_code_id = router.store_code(pcl_contract);

        let coin_registry_contract = Box::new(ContractWrapper::new_with_empty(
            oroswap_native_coin_registry::contract::execute,
            oroswap_native_coin_registry::contract::instantiate,
            oroswap_native_coin_registry::contract::query,
        ));
        let coin_registry_code_id = router.store_code(coin_registry_contract);

        let coin_registry = router
            .instantiate_contract(
                coin_registry_code_id,
                owner.clone(),
                &oroswap::native_coin_registry::InstantiateMsg {
                    owner: owner.to_string(),
                },
                &[],
                "coin_registry",
                None,
            )
            .unwrap();

        let factory_contract = Box::new(
            ContractWrapper::new_with_empty(
                oroswap_factory::contract::execute,
                oroswap_factory::contract::instantiate,
                oroswap_factory::contract::query,
            )
            .with_reply_empty(oroswap_factory::contract::reply),
        );

        let factory_code_id = router.store_code(factory_contract);

        let msg = oroswap::factory::InstantiateMsg {
            pair_configs: vec![
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Xyk {},
                    total_fee_bps: 0,
                    maker_fee_bps: 0,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                    pool_creation_fee: Uint128::new(1000),
                },
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Stable {},
                    total_fee_bps: 0,
                    maker_fee_bps: 0,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                    pool_creation_fee: Uint128::new(1000),
                },
                PairConfig {
                    code_id: pcl_code_id,
                    maker_fee_bps: 5000,
                    total_fee_bps: 0u16, // Concentrated pair does not use this field,
                    pair_type: PairType::Custom("concentrated".to_string()),
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                    pool_creation_fee: Uint128::new(1000),
                },
            ],
            token_code_id: cw20_token_code_id,
            fee_address: Some(Addr::unchecked("maker").to_string()),
            generator_address: None,
            owner: owner.to_string(),
            whitelist_code_id: 0,
            coin_registry_address: coin_registry.to_string(),
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
            factory,
            coin_registry,
            cw20_token_code_id,
        }
    }

    pub fn create_pair(
        &mut self,
        app: &mut App,
        _sender: &Addr,
        pair_type: PairType,
        asset_infos: [AssetInfo; 2],
        init_params: Option<Binary>,
    ) -> AnyResult<Addr> {
        let permissionless = Addr::unchecked("permissionless");
        app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &permissionless, vec![coin(1000, "uzig")])
        })?;

        // Register native tokens with coin registry
        for asset_info in &asset_infos {
            if let AssetInfo::NativeToken { denom } = asset_info {
                app.execute_contract(
                    self.owner.clone(),
                    self.coin_registry.clone(),
                    &oroswap::native_coin_registry::ExecuteMsg::Add {
                        native_coins: vec![(denom.to_string(), 6)],
                    },
                    &[],
                )?;
            }
        }

        app.execute_contract(
            permissionless,
            self.factory.clone(),
            &FactoryExecuteMsg::CreatePair {
                pair_type: pair_type.clone(),
                asset_infos: asset_infos.to_vec(),
                init_params,
            },
            &[coin(1000, "uzig")],
        )?;

        let pair_info = app
            .wrap()
            .query_wasm_smart::<PairInfo>(
                &self.factory,
                &FactoryQueryMsg::Pair {
                    asset_infos: asset_infos.to_vec(),
                    pair_type,
                },
            )
            .unwrap();

        Ok(pair_info.contract_addr)
    }
}

pub fn instantiate_token(
    app: &mut App,
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

pub fn mint(
    app: &mut App,
    owner: &Addr,
    token: &Addr,
    amount: u128,
    receiver: &Addr,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        owner.clone(),
        token.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: receiver.to_string(),
            amount: amount.into(),
        },
        &[],
    )
}

pub fn mint_native(
    app: &mut App,
    denom: &str,
    amount: u128,
    receiver: &Addr,
) -> AnyResult<AppResponse> {
    // .init_balance() erases previous balance thus we use such hack and create intermediate "denom admin"
    let denom_admin = Addr::unchecked(format!("{denom}_admin"));
    let coins_vec = coins(amount, denom);
    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &denom_admin, coins_vec.clone())
    })
    .unwrap();

    app.send_tokens(denom_admin, receiver.clone(), &coins_vec)
}

pub fn mock_app() -> App {
    let mut app = AppBuilder::new()
        .with_stargate(MockStargate::default())
        .build(|_, _, _| {});

    // Add initial balances for test accounts
    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked("owner"), coins(10000, "uzig"))
    })
    .unwrap();

    app
}
