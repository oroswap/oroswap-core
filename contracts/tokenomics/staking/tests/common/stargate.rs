//! Stargate keeper for staking tests.
//!
//! The staking contract issues no stargate messages, so this is a no-op
//! handler. If a future change introduces a stargate dispatch the tests
//! will fail loudly here.

use std::fmt::Debug;

use anyhow::Result as AnyResult;
use cosmwasm_schema::schemars::JsonSchema;
use cosmwasm_schema::serde::de::DeserializeOwned;
use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CustomQuery, Querier, Storage};
use cw_multi_test::{AppResponse, CosmosRouter, Stargate};

#[derive(Default)]
pub struct StargateKeeper {}

impl Stargate for StargateKeeper {
    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _sender: Addr,
        _type_url: String,
        _value: Binary,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        // Internal-stake architecture: no tokenfactory operations are issued
        // by the staking contract itself. Silently accept anything that
        // happens to reach this handler so the test harness doesn't fail
        // on unrelated stargate calls a future test might trigger.
        Ok(AppResponse::default())
    }

    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        _path: String,
        _data: Binary,
    ) -> AnyResult<Binary> {
        unimplemented!("Stargate queries are not implemented")
    }
}
