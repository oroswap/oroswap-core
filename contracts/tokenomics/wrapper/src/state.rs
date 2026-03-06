use cosmwasm_schema::cw_serde;
use cw_storage_plus::Item;

/// Contract configuration stored on-chain
#[cw_serde]
pub struct Config {
    /// The token to be locked — can be any native denom:
    /// plain native (e.g. "uzig"), IBC (e.g. "ibc/F1073E..."), or
    /// a tokenfactory coin (e.g. "coin.xxx.yyy")
    pub lock_denom: String,
    /// The tokenfactory token minted 1:1 for each locked token
    /// (e.g. "coin.<contract_address>.<mint_subdenom>").
    /// Set to "" initially and populated in the reply handler.
    pub mint_denom: String,
    /// Owner / admin address
    pub owner: String,
    /// Whether the contract is currently paused
    pub paused: bool,
}

pub const CONFIG: Item<Config> = Item::new("config");
