use cosmwasm_std::{Addr, Uint128};
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
    /// IBC channel on Zigchain for the outbound first hop (typically Zigchain → Noble).
    pub ibc_channel: String,
    /// IBC packet timeout in seconds.
    pub ibc_timeout_secs: u64,

    // ── Destination policy ───────────────────────────────────────────────
    // Both fields are owner-set via UpdateConfig and used verbatim by
    // bridge_usdc — the keeper has no input into either. This is the
    // single property that bounds a hijacked keeper's blast radius to
    // "trigger only", not "redirect funds".

    /// First-hop receiver address (typically a Noble address). Inserted
    /// verbatim into the outbound MsgTransfer's `receiver` field.
    pub first_hop_receiver: String,

    /// JSON memo inserted verbatim into the outbound MsgTransfer's `memo`
    /// field. Carries PFM / CCTP forwarding instructions; same shape on
    /// testnet (e.g. forward to Osmosis) and mainnet (forward / CCTP to
    /// Ethereum) — only the contents differ per environment.
    pub forward_memo: String,

    /// Maximum USDC that can be bridged in a single `BridgeUsdc` call.
    /// `None` = uncapped. Bounds the blast radius of a hijacked keeper —
    /// they can still trigger transfers to the configured treasury, but
    /// only this much per tx.
    pub max_bridge_per_tx: Option<Uint128>,

    /// Whether bridging operations are paused.
    pub paused: bool,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
pub const KEEPERS: Map<&Addr, bool> = Map::new("keepers");

/// Context for the most recent `BridgeUsdc` call. Written before the
/// outbound IBC submsg is fired and consumed by the reply handler. With
/// `SubMsg::reply_always` the slot is always read+cleared in the same
/// transaction, so it never persists across blocks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BridgeAttempt {
    pub amount: Uint128,
    pub receiver: String,
    pub channel: String,
    pub denom: String,
}

pub const LAST_BRIDGE_ATTEMPT: Item<BridgeAttempt> = Item::new("last_bridge_attempt");
