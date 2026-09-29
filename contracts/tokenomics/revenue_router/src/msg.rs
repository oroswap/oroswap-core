use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    /// Contract owner.
    pub owner: String,
    /// USDC IBC denom on Zigchain (e.g. "ibc/6490A7...").
    pub usdc_denom: String,
    /// Outbound IBC channel on Zigchain (e.g. "channel-0", typically to Noble).
    pub ibc_channel: String,
    /// IBC packet timeout in seconds (default: 600).
    pub ibc_timeout_secs: Option<u64>,
    /// First-hop receiver — used verbatim as the outbound MsgTransfer
    /// `receiver`. Typically a Noble address (`noble1...`).
    pub first_hop_receiver: String,
    /// JSON memo carrying PFM / CCTP forwarding instructions. Inserted
    /// verbatim into the outbound MsgTransfer's `memo` field.
    pub forward_memo: String,
    /// Maximum USDC per `BridgeUsdc` call. `None` = uncapped.
    pub max_bridge_per_tx: Option<Uint128>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Bridge USDC out of this contract to the destination encoded in
    /// `Config.first_hop_receiver` + `Config.forward_memo`. Callable by
    /// the owner or any registered keeper.
    ///
    /// The caller can only choose how much to bridge — not where. The
    /// destination is owner-controlled policy in Config; a hijacked keeper
    /// can only trigger transfers to the configured treasury.
    BridgeUsdc {
        /// Amount to bridge. If None, bridges the full contract balance.
        amount: Option<Uint128>,
    },

    /// Update contract configuration. Owner only.
    ///
    /// `max_bridge_per_tx`:
    ///   - omitted / null  → cap left unchanged
    ///   - Some(Some(x))   → set cap to x
    ///   - Some(None)      → clear the cap (uncapped)
    UpdateConfig {
        usdc_denom: Option<String>,
        ibc_channel: Option<String>,
        ibc_timeout_secs: Option<u64>,
        first_hop_receiver: Option<String>,
        forward_memo: Option<String>,
        max_bridge_per_tx: Option<Option<Uint128>>,
    },

    /// Add an authorized keeper. Owner only.
    AddKeeper { keeper: String },

    /// Remove a keeper. Owner only.
    RemoveKeeper { keeper: String },

    /// Recover any token from the contract. Owner only.
    EmergencyWithdraw {
        denom: String,
        amount: Option<Uint128>,
        recipient: Option<String>,
    },

    /// Proposes a new owner. The proposed owner must call ClaimOwnership.
    ProposeNewOwner { owner: String, expires_in: u64 },

    /// Removes the pending ownership proposal.
    DropOwnershipProposal {},

    /// Claims a pending ownership proposal. Only callable by the proposed new owner.
    ClaimOwnership {},

    /// Pause or unpause bridge operations. Owner only.
    SetPaused { paused: bool },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    #[returns(BalanceResponse)]
    UsdcBalance {},

    /// Paginated list of authorised keepers.
    ///
    /// `start_after`: skip keys ≤ this bech32 (exclusive).
    /// `limit`: max entries to return (default 30, capped at 100).
    #[returns(KeepersResponse)]
    Keepers {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns exactly what the next `BridgeUsdc` call would send to
    /// chain — owner sanity-check before enabling the keeper.
    #[returns(PreviewBridgeResponse)]
    PreviewBridge {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub usdc_denom: String,
    pub ibc_channel: String,
    pub ibc_timeout_secs: u64,
    pub first_hop_receiver: String,
    pub forward_memo: String,
    pub max_bridge_per_tx: Option<Uint128>,
    pub paused: bool,
}

#[cw_serde]
pub struct BalanceResponse {
    pub denom: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct KeepersResponse {
    pub keepers: Vec<String>,
}

/// What the contract would put on chain on the next BridgeUsdc call.
/// The keeper cannot affect any of these values.
#[cw_serde]
pub struct PreviewBridgeResponse {
    pub channel: String,
    pub denom: String,
    pub receiver: String,
    pub memo: String,
    pub timeout_secs: u64,
    pub max_bridge_per_tx: Option<Uint128>,
}

#[cw_serde]
pub struct MigrateMsg {}
