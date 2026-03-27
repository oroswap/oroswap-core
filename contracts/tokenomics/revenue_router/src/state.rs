use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DEFAULT_IBC_TIMEOUT_SECS: u64 = 600; // 10 minutes

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Contract owner.
    pub owner: Addr,
    /// USDC IBC denom on Zigchain.
    pub usdc_denom: String,
    /// Destination chain ID (Ethereum mainnet = "1").
    pub dest_chain_id: String,
    /// USDC denom on destination chain.
    pub dest_usdc_denom: String,
    /// Final receiver address on Ethereum.
    pub dest_receiver: String,
    /// IBC channel on Zigchain for outbound transfers.
    pub ibc_channel: String,
    /// IBC packet timeout in seconds.
    pub ibc_timeout_secs: u64,
    /// Authorized keeper addresses.
    pub keepers: Vec<Addr>,
}

pub const CONFIG: Item<Config> = Item::new("config");
