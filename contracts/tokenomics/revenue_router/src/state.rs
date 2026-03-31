use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use oroswap::common::OwnershipProposal;

pub const DEFAULT_IBC_TIMEOUT_SECS: u64 = 600; // 10 minutes
pub const MAX_IBC_TIMEOUT_SECS: u64 = 86400; // 24 hours

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Contract owner.
    pub owner: Addr,
    /// USDC IBC denom on Zigchain.
    pub usdc_denom: String,
    /// IBC channel on Zigchain for outbound transfers.
    pub ibc_channel: String,
    /// IBC packet timeout in seconds.
    pub ibc_timeout_secs: u64,
    /// Whether bridging operations are paused.
    pub paused: bool,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
pub const KEEPERS: Map<&Addr, bool> = Map::new("keepers");
