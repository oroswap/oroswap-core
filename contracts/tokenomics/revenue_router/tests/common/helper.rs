#![allow(dead_code)]

use anyhow::Result as AnyResult;
use cosmwasm_std::{coins, Addr, Coin, StdResult, Uint128};
use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor};

use oroswap_revenue_router::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, KeepersResponse, PreviewBridgeResponse, QueryMsg,
};

pub const USDC_DENOM: &str = "ibc/5260516290F7883EC893AADA09A6B8CEC790F2EEF3196F440037908749785BE8";
pub const FIRST_HOP_RECEIVER: &str = "noble1receiver000000000000000000000000000000";
pub const FORWARD_MEMO: &str = r#"{"forward":{"channel":"channel-22","port":"transfer","receiver":"osmo1abc","retries":2,"timeout":"10m"}}"#;

pub struct Helper {
    pub app: App,
    pub owner: Addr,
    pub contract: Addr,
}

fn router_contract() -> Box<dyn Contract<cosmwasm_std::Empty>> {
    Box::new(
        ContractWrapper::new_with_empty(
            oroswap_revenue_router::contract::execute,
            oroswap_revenue_router::contract::instantiate,
            oroswap_revenue_router::contract::query,
        )
        .with_reply_empty(oroswap_revenue_router::contract::reply)
        .with_migrate_empty(oroswap_revenue_router::contract::migrate),
    )
}

impl Helper {
    pub fn new(owner: &Addr) -> AnyResult<Self> {
        Self::new_with_cap(owner, None)
    }

    pub fn new_with_cap(owner: &Addr, max_bridge_per_tx: Option<Uint128>) -> AnyResult<Self> {
        let mut app = App::new(|router, _, storage| {
            router
                .bank
                .init_balance(storage, owner, coins(u128::MAX, USDC_DENOM))
                .unwrap();
        });

        let code_id = app.store_code(router_contract());

        let msg = InstantiateMsg {
            owner: owner.to_string(),
            usdc_denom: USDC_DENOM.to_string(),
            ibc_channel: "channel-44".to_string(),
            ibc_timeout_secs: None,
            first_hop_receiver: FIRST_HOP_RECEIVER.to_string(),
            forward_memo: FORWARD_MEMO.to_string(),
            max_bridge_per_tx,
        };

        let contract = app.instantiate_contract(
            code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("oroswap-revenue-router"),
            Some(owner.to_string()),
        )?;

        Ok(Self { app, owner: owner.clone(), contract })
    }

    // ── Funding ──────────────────────────────────────────────────────────────

    pub fn fund_contract(&mut self, amount: u128) {
        self.app
            .send_tokens(self.owner.clone(), self.contract.clone(), &coins(amount, USDC_DENOM))
            .unwrap();
    }

    pub fn fund(&mut self, recipient: &Addr, c: Coin) {
        self.app.send_tokens(self.owner.clone(), recipient.clone(), &[c]).unwrap();
    }

    // ── Execute helpers ──────────────────────────────────────────────────────

    pub fn bridge(&mut self, sender: &Addr, amount: Option<u128>) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::BridgeUsdc { amount: amount.map(Uint128::new) },
            &[],
        )
    }

    pub fn add_keeper(&mut self, sender: &Addr, keeper: &str) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::AddKeeper { keeper: keeper.to_string() },
            &[],
        )
    }

    pub fn remove_keeper(&mut self, sender: &Addr, keeper: &str) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::RemoveKeeper { keeper: keeper.to_string() },
            &[],
        )
    }

    pub fn set_paused(&mut self, sender: &Addr, paused: bool) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::SetPaused { paused },
            &[],
        )
    }

    pub fn emergency_withdraw(
        &mut self,
        sender: &Addr,
        denom: &str,
        amount: Option<u128>,
        recipient: Option<&str>,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::EmergencyWithdraw {
                denom: denom.to_string(),
                amount: amount.map(Uint128::new),
                recipient: recipient.map(str::to_string),
            },
            &[],
        )
    }

    pub fn propose_new_owner(
        &mut self,
        sender: &Addr,
        new_owner: &Addr,
        expires_in: u64,
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::ProposeNewOwner {
                owner: new_owner.to_string(),
                expires_in,
            },
            &[],
        )
    }

    pub fn claim_ownership(&mut self, sender: &Addr) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender.clone(),
            self.contract.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
    }

    // ── Query helpers ────────────────────────────────────────────────────────

    pub fn query_config(&self) -> StdResult<ConfigResponse> {
        self.app.wrap().query_wasm_smart(&self.contract, &QueryMsg::Config {})
    }

    pub fn query_keepers(&self, start_after: Option<&str>, limit: Option<u32>) -> StdResult<KeepersResponse> {
        self.app.wrap().query_wasm_smart(
            &self.contract,
            &QueryMsg::Keepers {
                start_after: start_after.map(str::to_string),
                limit,
            },
        )
    }

    pub fn query_preview(&self) -> StdResult<PreviewBridgeResponse> {
        self.app.wrap().query_wasm_smart(&self.contract, &QueryMsg::PreviewBridge {})
    }

    pub fn query_balance(&self, addr: &Addr, denom: &str) -> StdResult<Uint128> {
        self.app.wrap().query_balance(addr, denom).map(|c| c.amount)
    }
}
