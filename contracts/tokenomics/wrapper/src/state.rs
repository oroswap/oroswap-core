use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use oroswap::common::OwnershipProposal;

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
    pub owner: Addr,
    /// Whether the contract is currently paused
    pub paused: bool,
}

/// Temporary mint metadata stored between instantiate and reply.
#[cw_serde]
pub struct MintMeta {
    /// Sub-denom suffix passed to MsgCreateDenom (e.g. "xoro").
    pub subdenom: String,
    /// Decimal precision used in the on-chain Metadata.
    pub decimals: u32,
    /// Optional URI for the on-chain Metadata.
    pub uri: Option<String>,
    /// Human-readable display name (e.g. "Wrapped ORO"). If `None` the
    /// metadata builder falls back to a default derived from `subdenom`.
    pub name: Option<String>,
    /// Ticker symbol shown by wallets (e.g. "xORO"). If `None` the builder
    /// derives one from `subdenom`.
    pub symbol: Option<String>,
    /// Free-form description for the on-chain Metadata.
    pub description: Option<String>,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// Temporary storage for mint metadata used between instantiate and reply.
/// Removed after the reply handler consumes it.
pub const MINT_META: Item<MintMeta> = Item::new("mint_meta");

/// Pending ownership proposal created via ProposeNewOwner.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// Addresses (in addition to the owner) authorised to pause the contract.
/// Only the owner can add/remove entries, and only the owner can unpause —
/// so a compromised authority can pause but cannot grief by toggling.
pub const PAUSE_AUTHORITIES: Map<&Addr, ()> = Map::new("pause_authorities");
