use cosmwasm_std::{Addr, Binary, Coin, Uint128};
use oroswap_test::cw_multi_test::{Executor, BankSudo};
use oroswap::asset::{AssetInfo, PairInfo};
use oroswap::factory::{ExecuteMsg, PairType, TrackerConfig, PairConfig};
use oroswap_test::modules::stargate::StargateApp;
use cw20::Cw20ExecuteMsg;

mod factory_helper;
use crate::factory_helper::{FactoryHelper, instantiate_token};

fn store_factory_code(app: &mut StargateApp) -> u64 {
    let factory_contract = Box::new(
        oroswap_test::cw_multi_test::ContractWrapper::new_with_empty(
            oroswap_factory::contract::execute,
            oroswap_factory::contract::instantiate,
            oroswap_factory::contract::query,
        )
        .with_reply_empty(oroswap_factory::contract::reply),
    );

    app.store_code(factory_contract)
}

