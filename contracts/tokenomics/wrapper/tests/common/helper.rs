#![allow(dead_code)]

use anyhow::Result as AnyResult;
use cosmwasm_std::{
    coins, Addr, Coin, Empty, GovMsg, IbcMsg, IbcQuery, MemoryStorage, StdResult, Uint128,
};
use cosmwasm_std::testing::MockApi;
use cw_multi_test::{
    App, AppResponse, BankKeeper, BasicAppBuilder, Contract, ContractWrapper, DistributionKeeper,
    Executor, FailingModule, StakeKeeper, WasmKeeper,
};

use oroswap_wrapper::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use oroswap_wrapper::state::Config;

use crate::common::stargate::StargateKeeper;

/// The mock IBC denom used as `lock_denom` in tests.
/// Simulates a real IBC token path.
pub const LOCK_DENOM: &str =
    "ibc/F1073E5895A25347EF10A7B50783D3048882726895AEE1176C498C45D0576F2B";

/// Sub-denom suffix for the minted token.
pub const MINT_SUBDENOM: &str = "xoro";

/// The type alias for our customised multi-test App with the Stargate keeper.
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

/// Convenience wrapper around the multi-test application for wrapper tests.
pub struct Helper {
    pub app: CustomizedApp,
    pub owner: Addr,
    pub wrapper: Addr,
    /// The full tokenfactory denom of the minted token.
    pub mint_denom: String,
}

fn wrapper_contract() -> Box<dyn Contract<Empty>> {
    Box::new(
        ContractWrapper::new_with_empty(
            oroswap_wrapper::contract::execute,
            oroswap_wrapper::contract::instantiate,
            oroswap_wrapper::contract::query,
        )
        .with_reply_empty(oroswap_wrapper::contract::reply),
    )
}

impl Helper {
    /// Instantiate the wrapper contract with the mock IBC `LOCK_DENOM`.
    /// The owner receives an initial balance of `LOCK_DENOM` tokens so tests
    /// can fund users.
    pub fn new(owner: &Addr) -> AnyResult<Self> {
        let mut app = BasicAppBuilder::new()
            .with_stargate(StargateKeeper::default())
            .build(|router, _, storage| {
                // Give the owner a large supply of the IBC token for distribution
                router
                    .bank
                    .init_balance(storage, owner, coins(u128::MAX, LOCK_DENOM))
                    .unwrap();
            });

        let wrapper_code_id = app.store_code(wrapper_contract());

        let msg = InstantiateMsg {
            lock_denom: LOCK_DENOM.to_string(),
            mint_subdenom: MINT_SUBDENOM.to_string(),
            mint_uri: Some("https://example.com/metadata.json".to_string()),
            mint_decimals: Some(6),
            owner: owner.to_string(),
        };

        let wrapper = app
            .instantiate_contract(
                wrapper_code_id,
                owner.clone(),
                &msg,
                &[], // no funds at instantiation
                String::from("Oroswap Wrapper"),
                Some(owner.to_string()),
            )
            .unwrap();

        let Config { mint_denom, .. } = app
            .wrap()
            .query_wasm_smart(&wrapper, &QueryMsg::Config {})
            .unwrap();

        Ok(Self {
            app,
            owner: owner.clone(),
            wrapper,
            mint_denom,
        })
    }

    // -----------------------------------------------------------------------
    // Funding helpers
    // -----------------------------------------------------------------------

    /// Send `amount` of `LOCK_DENOM` from the owner to `recipient`.
    pub fn give_lock_tokens(&mut self, amount: u128, recipient: &Addr) {
        self.app
            .send_tokens(
                self.owner.clone(),
                recipient.clone(),
                &coins(amount, LOCK_DENOM),
            )
            .unwrap();
    }

    /// Mint an arbitrary coin to an address (for testing wrong denoms etc.)
    pub fn mint_coin(&mut self, to: &Addr, coin: Coin) {
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

    // -----------------------------------------------------------------------
    // Execute helpers
    // -----------------------------------------------------------------------

    /// Lock `amount` of `LOCK_DENOM` from `sender`, minting to sender.
    pub fn lock(&mut self, sender: &Addr, amount: u128) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.wrapper.clone(),
            &ExecuteMsg::Lock { receiver: None },
            &coins(amount, LOCK_DENOM),
        )
    }

    /// Lock `amount` of `LOCK_DENOM` from `sender`, sending minted tokens to `receiver`.
    pub fn lock_to(&mut self, sender: &Addr, amount: u128, receiver: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.wrapper.clone(),
            &ExecuteMsg::Lock {
                receiver: Some(receiver.to_string()),
            },
            &coins(amount, LOCK_DENOM),
        )
    }

    /// Unlock `amount` of mint tokens from `sender`, returning lock tokens to sender.
    pub fn unlock(&mut self, sender: &Addr, amount: u128) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.wrapper.clone(),
            &ExecuteMsg::Unlock { receiver: None },
            &coins(amount, &self.mint_denom),
        )
    }

    /// Unlock `amount` of mint tokens from `sender`, returning lock tokens to `receiver`.
    pub fn unlock_to(&mut self, sender: &Addr, amount: u128, receiver: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.wrapper.clone(),
            &ExecuteMsg::Unlock {
                receiver: Some(receiver.to_string()),
            },
            &coins(amount, &self.mint_denom),
        )
    }

    /// Pause the contract (owner only).
    pub fn pause(&mut self, sender: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.wrapper.clone(),
            &ExecuteMsg::Pause {},
            &[],
        )
    }

    /// Unpause the contract (owner only).
    pub fn unpause(&mut self, sender: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.wrapper.clone(),
            &ExecuteMsg::Unpause {},
            &[],
        )
    }

    // -----------------------------------------------------------------------
    // Query helpers
    // -----------------------------------------------------------------------

    pub fn query_balance(&self, addr: &Addr, denom: &str) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_balance(addr, denom)
            .map(|c| c.amount)
    }

    pub fn query_total_locked(&self) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_wasm_smart(&self.wrapper, &QueryMsg::TotalLocked {})
    }

    pub fn query_config(&self) -> StdResult<Config> {
        self.app
            .wrap()
            .query_wasm_smart(&self.wrapper, &QueryMsg::Config {})
    }

    pub fn next_block(&mut self, seconds: u64) {
        self.app.update_block(|block| {
            block.time = block.time.plus_seconds(seconds);
            block.height += 1;
        });
    }
}
