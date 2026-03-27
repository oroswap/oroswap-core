use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    /// Contract owner.
    pub owner: String,
    /// USDC IBC denom on Zigchain (e.g. "ibc/6490A7...").
    pub usdc_denom: String,
    /// Destination chain ID (e.g. "1" for Ethereum mainnet).
    pub dest_chain_id: String,
    /// USDC contract address on Ethereum.
    pub dest_usdc_denom: String,
    /// Final receiver address on Ethereum.
    pub dest_receiver: String,
    /// Outbound IBC channel on Zigchain (e.g. "channel-0").
    pub ibc_channel: String,
    /// IBC packet timeout in seconds (default: 600).
    pub ibc_timeout_secs: Option<u64>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Bridge USDC from this contract to Ethereum via Skip IBC routing.
    ///
    /// The keeper calls Skip API off-chain to get the route + PFM memo, then
    /// passes them here. The contract fires a single MsgTransfer; Skip's
    /// relayer handles the multi-hop (Zigchain → Noble → Ethereum).
    BridgeUsdc {
        /// Amount to bridge. If None, bridges the full contract balance.
        amount: Option<Uint128>,
        /// PFM memo from Skip API `/v2/fungible/msgs`.
        memo: String,
        /// First-hop receiver address (from Skip API address_list).
        receiver: String,
    },

    /// Update contract configuration. Owner only.
    UpdateConfig {
        owner: Option<String>,
        usdc_denom: Option<String>,
        dest_chain_id: Option<String>,
        dest_usdc_denom: Option<String>,
        dest_receiver: Option<String>,
        ibc_channel: Option<String>,
        ibc_timeout_secs: Option<u64>,
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
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},

    #[returns(BalanceResponse)]
    UsdcBalance {},

    #[returns(KeepersResponse)]
    Keepers {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub usdc_denom: String,
    pub dest_chain_id: String,
    pub dest_usdc_denom: String,
    pub dest_receiver: String,
    pub ibc_channel: String,
    pub ibc_timeout_secs: u64,
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

#[cw_serde]
pub struct MigrateMsg {}
