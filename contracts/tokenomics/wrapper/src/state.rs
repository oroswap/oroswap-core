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
    /// Decimal precision of the minted denom, baked in once at
    /// `MsgCreateDenom` time. Persisted so `UpdateDenomMetadata` can echo
    /// the correct exponent — the chain's Metadata stores DenomUnits, not
    /// a free-standing decimals field, and rewriting them with the wrong
    /// exponent would mis-state precision in every wallet/explorer.
    pub mint_decimals: u32,
    /// Owner / admin address
    pub owner: Addr,
    /// Whether the contract is currently paused
    pub paused: bool,
}

/// Authoritative copy of the minted denom's human-readable Metadata.
///
/// The ZigChain tokenfactory module overwrites Metadata as a whole on each
/// `MsgSetDenomMetadata`, so a "patch-style" `UpdateDenomMetadata` call must
/// load the current values, overlay any `Some(_)` fields from the caller,
/// and re-encode the full Metadata. Persisting the resolved values here
/// keeps that patch behaviour honest — without it, every update would have
/// to either (a) require the caller to repeat every field they want to
/// preserve, or (b) silently fall back to instantiate-time defaults and
/// wipe custom branding. We do (c): merge against this stored copy.
#[cw_serde]
pub struct DenomMetadata {
    /// Human-readable name (e.g. "Wrapped ORO").
    pub name: String,
    /// Ticker symbol shown by wallets (e.g. "xORO").
    pub symbol: String,
    /// Free-form description.
    pub description: String,
    /// Optional URI for the on-chain Metadata (logo, manifest, etc.).
    pub uri: Option<String>,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// Latest resolved on-chain Metadata for the minted denom. Written at
/// instantiate (defaults resolved from subdenom) and overwritten by
/// `UpdateDenomMetadata` patches.
pub const DENOM_METADATA: Item<DenomMetadata> = Item::new("denom_metadata");

/// Pending ownership proposal created via ProposeNewOwner.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// Addresses (in addition to the owner) authorised to pause the contract.
/// Only the owner can add/remove entries, and only the owner can unpause —
/// so a compromised authority can pause but cannot grief by toggling.
pub const PAUSE_AUTHORITIES: Map<&Addr, ()> = Map::new("pause_authorities");
