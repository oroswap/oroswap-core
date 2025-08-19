use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, Decimal, Uint128};
use oroswap_core::asset::{Asset, AssetInfo};
use oroswap_core::factory::PairType;

/// This structure describes the parameters used for creating a contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// The factory contract address
    pub factory_addr: String,
    /// The fee required for creating a pair (in uzig)
    pub pair_creation_fee: Uint128,
}

/// This structure describes the execute messages available in the contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Create a pair and provide liquidity in a single transaction
    CreatePairAndProvideLiquidity {
        /// The pair type (XYK, Stable, etc.)
        pair_type: PairType,
        /// Information about assets in the pool
        asset_infos: Vec<AssetInfo>,
        /// Optional binary serialised parameters for custom pool types
        init_params: Option<Binary>,
        /// Liquidity parameters
        liquidity: ProvideLiquidityParams,
    },
    /// Update the contract configuration (admin only)
    UpdateConfig {
        /// New factory address (optional)
        factory_addr: Option<String>,
        /// New pair creation fee (optional)
        pair_creation_fee: Option<Uint128>,
    },
}

/// Liquidity parameters for providing liquidity
#[cw_serde]
pub struct ProvideLiquidityParams {
    pub assets: Vec<Asset>,
    pub slippage_tolerance: Option<Decimal>,
    pub auto_stake: Option<bool>,
    pub receiver: Option<String>,
    pub min_lp_to_receive: Option<Uint128>,
}

/// This structure describes the query messages available in the contract.
#[cw_serde]
pub enum QueryMsg {
    /// Get the contract configuration
    Config {},
}

/// This structure describes the response to a config query.
#[cw_serde]
pub struct ConfigResponse {
    pub factory_addr: String,
    pub pair_creation_fee: Uint128,
}
