use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use crate::msg::ProvideLiquidityParams;

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = "oroswap-pool-initializer";
/// Contract version that is used for migration.
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Configuration for the pool initializer contract
#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub factory_addr: Addr,
    pub pair_creation_fee: Uint128,
    pub fee_denom: String,
}

/// Pending liquidity operation data
#[cw_serde]
pub struct PendingLiquidity {
    pub sender: Addr,
    pub pair_type: oroswap_core::factory::PairType,
    pub asset_infos: Vec<oroswap_core::asset::AssetInfo>,
    pub init_params: Option<cosmwasm_std::Binary>,
    pub liquidity: ProvideLiquidityParams,
    pub funds: Vec<cosmwasm_std::Coin>, // Store the native token funds
    pub cw20_messages: Vec<(Addr, Uint128)>, // Store CW-20 messages to send
}

// Reply IDs
pub const CREATE_PAIR_REPLY_ID: u64 = 1;
pub const PROVIDE_LIQUIDITY_REPLY_ID: u64 = 2;

// Storage items
pub const CONFIG: Item<Config> = Item::new("config");
pub const PENDING_LIQUIDITY: Item<PendingLiquidity> = Item::new("pending_liquidity");
