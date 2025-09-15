#![allow(dead_code)]

use anyhow::Result as AnyResult;
use cosmwasm_schema::serde::Serialize;
use cosmwasm_std::testing::MockApi;
use cosmwasm_std::{
    coins, to_json_binary, Addr, Coin, DepsMut, Empty, Env, GovMsg, IbcMsg, IbcQuery,
    MemoryStorage, MessageInfo, Response, StdResult, Uint128,
};
use cw_multi_test::{
    App, AppResponse, BankKeeper, BasicAppBuilder, Contract, ContractWrapper, DistributionKeeper,
    Executor, FailingModule, StakeKeeper, WasmKeeper, TOKEN_FACTORY_MODULE,
};

use oroswap::staking::{Config, ExecuteMsg, InstantiateMsg, QueryMsg, TrackerData};

use crate::common::stargate::StargateKeeper;

fn staking_contract() -> Box<dyn Contract<Empty>> {
    Box::new(
        ContractWrapper::new_with_empty(
            oroswap_staking::contract::execute,
            oroswap_staking::contract::instantiate,
            oroswap_staking::contract::query,
        )
        .with_reply_empty(oroswap_staking::contract::reply),
    )
}

fn tracker_contract() -> Box<dyn Contract<Empty>> {
    Box::new(
        ContractWrapper::new_with_empty(
            |_: DepsMut, _: Env, _: MessageInfo, _: Empty| -> StdResult<Response> {
                unimplemented!()
            },
            oroswap_tokenfactory_tracker::contract::instantiate,
            oroswap_tokenfactory_tracker::query::query,
        )
        .with_sudo_empty(oroswap_tokenfactory_tracker::contract::sudo),
    )
}

pub type CustomizedApp = App<
    BankKeeper,
    MockApi,
    MemoryStorage,
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    StakeKeeper,
    DistributionKeeper,
    FailingModule<IbcMsg, IbcQuery, Empty>,
    FailingModule<GovMsg, Empty, Empty>,
    StargateKeeper,
>;

pub struct Helper {
    pub app: CustomizedApp,
    pub owner: Addr,
    pub staking: Addr,
    pub tracker_addr: String,
    pub xoro_denom: String,
}

pub const ORO_DENOM: &str = "coin.assembly.oro";

impl Helper {
    pub fn new(owner: &Addr) -> AnyResult<Self> {
        let mut app = BasicAppBuilder::new()
            .with_stargate(StargateKeeper::default())
            .build(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, owner, coins(u128::MAX, ORO_DENOM))
                    .unwrap()
            });

        let staking_code_id = app.store_code(staking_contract());
        let tracker_code_id = app.store_code(tracker_contract());

        let msg = InstantiateMsg {
            deposit_token_denom: ORO_DENOM.to_string(),
            tracking_admin: owner.to_string(),
            tracking_code_id: tracker_code_id,
            token_factory_addr: TOKEN_FACTORY_MODULE.to_string(),
            bootstrap_amount: None, // Use default MINIMUM_STAKE_AMOUNT
        };
        let staking = app
            .instantiate_contract(
                staking_code_id,
                owner.clone(),
                &msg,
                &coins(1000, ORO_DENOM), // Bootstrap amount
                String::from("Oroswap Staking"),
                None,
            )
            .unwrap();

        let TrackerData { tracker_addr, .. } = app
            .wrap()
            .query_wasm_smart(&staking, &QueryMsg::TrackerConfig {})
            .unwrap();
        let Config { xoro_denom, .. } = app
            .wrap()
            .query_wasm_smart(&staking, &QueryMsg::Config {})
            .unwrap();

        Ok(Self {
            app,
            owner: owner.clone(),
            staking,
            tracker_addr,
            xoro_denom,
        })
    }

    pub fn give_oro(&mut self, amount: u128, recipient: &Addr) {
        self.app
            .send_tokens(
                self.owner.clone(),
                recipient.clone(),
                &coins(amount, ORO_DENOM),
            )
            .unwrap();
    }

    pub fn stake(&mut self, sender: &Addr, amount: u128) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Enter { receiver: None },
            &coins(amount, ORO_DENOM),
        )
    }

    pub fn stake_with_hook<T: Serialize + ?Sized>(
        &mut self,
        sender: &Addr,
        amount: u128,
        contract_address: String,
        msg: &T,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::EnterWithHook {
                contract_address,
                msg: to_json_binary(msg)?,
            },
            &coins(amount, ORO_DENOM),
        )
    }

    pub fn unstake(&mut self, sender: &Addr, amount: u128) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Leave { receiver: None },
            &coins(amount, &self.xoro_denom),
        )
    }

    pub fn query_balance(&self, sender: &Addr, denom: &str) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_balance(sender, denom)
            .map(|c| c.amount)
    }

    pub fn query_xoro_balance_at(
        &self,
        sender: &Addr,
        timestamp: Option<u64>,
    ) -> StdResult<Uint128> {
        self.app.wrap().query_wasm_smart(
            &self.staking,
            &QueryMsg::BalanceAt {
                address: sender.to_string(),
                timestamp,
            },
        )
    }

    pub fn query_xoro_supply_at(&self, timestamp: Option<u64>) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_wasm_smart(&self.staking, &QueryMsg::TotalSupplyAt { timestamp })
    }

    pub fn mint_coin(&mut self, to: &Addr, coin: Coin) {
        // .init_balance() erases previous balance thus I use such hack and create intermediate "denom admin"
        let denom_admin = Addr::unchecked(format!("{}_admin", &coin.denom));
        self.app
            .init_modules(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, &denom_admin, vec![coin.clone()])
            })
            .unwrap();

        self.app
            .send_tokens(denom_admin, to.clone(), &[coin])
            .unwrap();
    }

    pub fn next_block(&mut self, time: u64) {
        self.app.update_block(|block| {
            block.time = block.time.plus_seconds(time);
            block.height += 1
        });
    }
}
