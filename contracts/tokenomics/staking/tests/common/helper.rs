#![allow(dead_code)]

//! Test harness for the 1:1 ORO stake-registry staking contract.
//!
//! Architecture invariants this harness assumes:
//!   * Instantiate takes NO funds and NO bootstrap parameter — `STAKES` and
//!     `TOTAL_STAKES` start at zero and grow only via `Enter`.
//!   * `Leave` carries a typed `amount` + `recipient` instead of funds.
//!   * `query_stake_of` / `query_total_shares` hit `QueryMsg::StakeOf` and
//!     `QueryMsg::TotalShares`.
//!   * `mint_coin` is preserved — it fabricates arbitrary bank denoms via
//!     cw-multi-test's bank module for testing reward streams.

use anyhow::Result as AnyResult;
use cosmwasm_std::testing::MockApi;
use cosmwasm_std::{
    coins, Addr, Coin, Empty, GovMsg, IbcMsg, IbcQuery, MemoryStorage, StdResult, Uint128,
};
use cw_multi_test::{
    App, AppResponse, BankKeeper, BasicAppBuilder, Contract, ContractWrapper, DistributionKeeper,
    Executor, FailingModule, StakeKeeper, WasmKeeper,
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
}

pub const ORO_DENOM: &str = "coin.assembly.oro";

/// Default unbonding period: 7 days. Same as production.
pub const UNBONDING_PERIOD: u64 = 7 * 24 * 60 * 60;

impl Helper {
    /// Default constructor: instantiates with `minimum_stake_amount = 1`
    /// (the smallest non-zero floor permitted by the contract). The 1-unit
    /// floor preserves the legacy behaviour of pre-floor tests that stake
    /// arbitrary small integer amounts.
    pub fn new(owner: &Addr) -> AnyResult<Self> {
        Self::new_with_minimum(owner, Uint128::new(1))
    }

    /// Variant that lets a test exercise a specific `minimum_stake_amount`.
    pub fn new_with_minimum(owner: &Addr, minimum_stake_amount: Uint128) -> AnyResult<Self> {
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
            owner: owner.to_string(),
            oro_denom: ORO_DENOM.to_string(),
            unbonding_period: UNBONDING_PERIOD,
            minimum_stake_amount,
        };
        let staking = app
            .instantiate_contract(
                staking_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("Oroswap Staking"),
                Some(owner.to_string()),
            )
            .unwrap();

        Ok(Self {
            app,
            owner: owner.clone(),
            staking,
        })
    }

    /// Returns the result of instantiating with the supplied `minimum_stake_amount`
    /// without unwrapping, so tests can assert on rejection paths.
    pub fn try_new_with_minimum(
        owner: &Addr,
        minimum_stake_amount: Uint128,
    ) -> AnyResult<Self> {
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
            owner: owner.to_string(),
            oro_denom: ORO_DENOM.to_string(),
            unbonding_period: UNBONDING_PERIOD,
            minimum_stake_amount,
        };
        let staking = app.instantiate_contract(
            staking_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("Oroswap Staking"),
            Some(owner.to_string()),
        )?;

        Ok(Self {
            app,
            owner: owner.clone(),
            staking,
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
            &ExecuteMsg::Enter {},
            &coins(amount, ORO_DENOM),
        )
    }

    /// Leave a typed `amount` of ORO stake to `recipient` (defaults to sender
    /// if `None`). Note: `Leave` carries no `info.funds`.
    pub fn unstake(&mut self, sender: &Addr, amount: u128) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Leave {
                amount: Uint128::new(amount),
                recipient: sender.to_string(),
            },
            &[],
        )
    }

    /// Same as `unstake` but lets the caller direct the matured ORO to a
    /// different recipient when `Claim` is eventually called.
    pub fn unstake_to(
        &mut self,
        sender: &Addr,
        amount: u128,
        recipient: &Addr,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::Leave {
                amount: Uint128::new(amount),
                recipient: recipient.to_string(),
            },
            &[],
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

    /// AddReward in the simplified interface: the message body now carries
    /// only `duration`. The reward denom and total amount come from
    /// `info.funds`; the contract derives RPS as `amount / duration`.
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

    /// Invokes `UpdateConfig` with optional new `unbonding_period` and/or
    /// `minimum_stake_amount`. Mirrors the JSON-level message shape so tests
    /// can exercise either field independently.
    pub fn update_config(
        &mut self,
        sender: &Addr,
        unbonding_period: Option<u64>,
        minimum_stake_amount: Option<Uint128>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::UpdateConfig {
                unbonding_period,
                minimum_stake_amount,
            },
            &[],
        )
    }

    pub fn add_pause_authorities(
        &mut self,
        sender: &Addr,
        addresses: Vec<String>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::AddPauseAuthorities { addresses },
            &[],
        )
    }

    pub fn remove_pause_authorities(
        &mut self,
        sender: &Addr,
        addresses: Vec<String>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::RemovePauseAuthorities { addresses },
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

    /// Sweep orphaned rewards (emissions that elapsed while no users were
    /// staked) to `recipient` — defaults to the contract owner if `None`.
    /// Returns the raw `AnyResult` so error-path tests can assert on
    /// `Unauthorized` / `NoOrphanedRewards`.
    pub fn claim_orphaned_rewards(
        &mut self,
        sender: &Addr,
        recipient: Option<&Addr>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.staking.clone(),
            &ExecuteMsg::ClaimOrphanedRewards {
                recipient: recipient.map(|a| a.to_string()),
            },
            &[],
        )
    }

    /// Direct read of `ORPHANED_REWARDS[denom]` via raw contract storage.
    /// Returns `Uint128::zero()` when no entry exists. Used by orphan-path
    /// tests to assert bucket contents without going through a query
    /// handler (the contract intentionally does NOT expose a query for this —
    /// the bucket is owner-recovery state, not user-facing).
    pub fn query_orphaned_rewards(&self, denom: &str) -> Uint128 {
        // Key layout for `Map<&str, Uint128>` with namespace "orphaned_rewards":
        //   length-prefixed("orphaned_rewards") || denom_bytes
        // where length-prefix = big-endian u16 length of namespace bytes.
        let ns = b"orphaned_rewards";
        let mut key = Vec::with_capacity(2 + ns.len() + denom.len());
        key.extend_from_slice(&(ns.len() as u16).to_be_bytes());
        key.extend_from_slice(ns);
        key.extend_from_slice(denom.as_bytes());

        let raw = self
            .app
            .wrap()
            .query_wasm_raw(&self.staking, key)
            .unwrap();
        match raw {
            Some(b) => cosmwasm_std::from_json(&b).unwrap(),
            None => Uint128::zero(),
        }
    }

    pub fn bank_send(
        &mut self,
        sender: &Addr,
        recipient: &Addr,
        amount: Vec<Coin>,
    ) -> AnyResult<AppResponse> {
        self.app
            .send_tokens(sender.clone(), recipient.clone(), &amount)
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

    /// Per-user stake balance via `QueryMsg::StakeOf { address }`.
    pub fn query_stake_of(&self, sender: &Addr) -> StdResult<Uint128> {
        self.app.wrap().query_wasm_smart(
            &self.staking,
            &QueryMsg::StakeOf {
                address: sender.to_string(),
            },
        )
    }

    /// Total stake supply (sum over `STAKES`) via `QueryMsg::TotalShares {}`.
    /// The legacy name is retained for backwards-compatible JSON shape; the
    /// returned value is equal to total staked ORO under the 1:1 model.
    pub fn query_total_shares(&self) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_wasm_smart(&self.staking, &QueryMsg::TotalShares {})
    }

    pub fn query_config(&self) -> StdResult<Config> {
        self.app
            .wrap()
            .query_wasm_smart(&self.staking, &QueryMsg::Config {})
    }

    pub fn query_total_locked(&self) -> StdResult<Uint128> {
        self.app
            .wrap()
            .query_wasm_smart(&self.staking, &QueryMsg::TotalLocked {})
    }

    /// Mints `coin` to `to` via cw-multi-test's bank module. Used to fabricate
    /// reward-denom coins (e.g. fake USDC) for AddReward — does *not* go
    /// through any tokenfactory mock, so it is safe under the internal-stake
    /// architecture.
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

    pub fn next_block(&mut self, time: u64) {
        self.app.update_block(|block| {
            block.time = block.time.plus_seconds(time);
            block.height += 1
        });
    }
}
