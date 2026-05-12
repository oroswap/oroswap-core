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

use oroswap::staking::{Config, ExecuteMsg, InstantiateMsg, QueryMsg};

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

        let msg = InstantiateMsg {
            deposit_token_denom: ORO_DENOM.to_string(),
            bootstrap_amount: None, // Use default MINIMUM_STAKE_AMOUNT
            unbonding_period: 7 * 24 * 60 * 60, // 7 days
            xoro_decimals: Some(6),
            xoro_uri: Some("https://example.com/metadata.json".to_string()),
        };
        let staking = app
            .instantiate_contract(
                staking_code_id,
                owner.clone(),
                &msg,
                &coins(1000, ORO_DENOM), // Bootstrap amount
                String::from("Oroswap Staking"),
                Some(owner.to_string()),
            )
            .unwrap();

        let Config { xoro_denom, .. } = app
            .wrap()
            .query_wasm_smart(&staking, &QueryMsg::Config {})
            .unwrap();

        Ok(Self {
            app,
            owner: owner.clone(),
            staking,
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

    pub fn claim(&mut self, sender: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Claim {},
            &[],
        )
    }

    pub fn pause(&mut self, sender: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Pause {},
            &[],
        )
    }

    pub fn unpause(&mut self, sender: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Unpause {},
            &[],
        )
    }

    pub fn add_reward(
        &mut self,
        sender: &Addr,
        coin: Coin,
        duration: u64,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::AddReward { duration },
            &[coin],
        )
    }

    pub fn add_pause_authorities(
        &mut self,
        sender: &Addr,
        authorities: Vec<String>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::AddPauseAuthorities { authorities },
            &[],
        )
    }

    pub fn remove_pause_authorities(
        &mut self,
        sender: &Addr,
        authorities: Vec<String>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::RemovePauseAuthorities { authorities },
            &[],
        )
    }

    pub fn query_pause_authorities(&self) -> StdResult<Vec<String>> {
        self.app
            .wrap()
            .query_wasm_smart(&self.staking, &QueryMsg::PauseAuthorities {})
    }

    pub fn withdraw_excess_tokens(
        &mut self,
        sender: &Addr,
        denom: &str,
        recipient: Option<String>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::WithdrawExcessTokens {
                denom: denom.to_string(),
                recipient,
            },
            &[],
        )
    }

    pub fn bank_send(
        &mut self,
        sender: &Addr,
        recipient: &Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<AppResponse> {
        self.app.send_tokens(sender.clone(), recipient.clone(), &amount)
    }

    pub fn query_pending_rewards(&self, address: &Addr) -> StdResult<Vec<Coin>> {
        self.app.wrap().query_wasm_smart(
            &self.staking,
            &QueryMsg::PendingRewards {
                address: address.to_string(),
            },
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

    pub fn query_total_locked(&self) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_wasm_smart(&self.staking, &QueryMsg::TotalLocked {})
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
