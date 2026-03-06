use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Uint128};

/// This structure describes the parameters used for creating a contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// The ORO token contract address
    pub deposit_token_denom: String,
    /// Token factory module address. Contract creator must ensure that the address is exact token factory module address.
    pub token_factory_addr: String,
    /// Bootstrap amount to initialize the pool (optional, defaults to MINIMUM_STAKE_AMOUNT)
    pub bootstrap_amount: Option<Uint128>,
    /// Unbonding period in seconds (e.g. 7 days = 604800)
    pub unbonding_period: u64,
    /// xORO decimals (optional, default: 6)
    pub xoro_decimals: Option<u32>,
    /// xORO metadata URI (optional)
    pub xoro_uri: Option<String>,
}

/// A single pending unbonding entry for a user.
#[cw_serde]
pub struct UnbondingEntry {
    /// The fixed ORO amount to be returned, calculated at the time of Leave.
    pub oro_amount: Uint128,
    /// Unix timestamp (in seconds) after which ORO can be claimed.
    pub unlock_time: u64,
}

/// This structure describes the execute messages available in the contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Deposits ORO in exchange for xORO
    /// The receiver is optional. If not set, the sender will receive the xORO.
    Enter { receiver: Option<String> },
    /// Deposits ORO in exchange for xORO
    /// and passes **all resulting xORO** to defined contract along with an executable message.
    EnterWithHook {
        contract_address: String,
        msg: Binary,
    },
    /// Burns xORO and begins unbonding of the proportional ORO amount.
    /// The receiver is optional (applied when Claim is called). If not set, the sender receives ORO on Claim.
    Leave { receiver: Option<String> },
    /// Claims all matured unbonding entries for the sender.
    /// Optional `receiver` overrides the destination set during Leave.
    Claim { receiver: Option<String> },
    /// Updates the contract configuration. Only callable by the owner.
    UpdateConfig {
        owner: Option<String>,
        unbonding_period: Option<u64>,
    },
    /// Pauses the contract. Only callable by the owner.
    Pause {},
    /// Unpauses the contract. Only callable by the owner.
    Unpause {},
}

/// This structure describes the query messages available in the contract.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// State returns the current pool state including exchange rate
    #[returns(StateResponse)]
    State {},
    /// Config returns the contract configuration specified in a custom [`Config`] structure
    #[returns(Config)]
    Config {},
    /// Returns xORO total supply. Duplicates TotalSupplyAt { timestamp: None } logic but kept for backward compatibility.
    #[returns(Uint128)]
    TotalShares {},
    /// Returns total ORO staked in the contract (excluding locked unbonding amounts).
    #[returns(Uint128)]
    TotalDeposit {},
    /// BalanceAt returns xORO balance of the given address at the given timestamp.
    /// Returns current balance if timestamp unset.
    #[returns(Uint128)]
    BalanceAt {
        address: String,
        timestamp: Option<u64>,
    },
    /// TotalSupplyAt returns xORO total token supply at the given timestamp.
    /// Returns current total supply if timestamp unset.
    #[returns(Uint128)]
    TotalSupplyAt { timestamp: Option<u64> },
    /// Returns total ORO currently in unbonding period.
    #[returns(Uint128)]
    TotalLocked {},
    /// Returns all pending unbonding entries for the given address.
    #[returns(Vec<UnbondingEntry>)]
    UnbondingEntries { address: String },
    /// Returns the configured unbonding period in seconds.
    #[returns(u64)]
    UnbondingPeriod {},
}

/// This structure stores the main parameters for the staking contract.
#[cw_serde]
pub struct Config {
    /// The ORO token denom
    pub oro_denom: String,
    /// The xORO token denom (coin.<contract_addr>.xORO)
    pub xoro_denom: String,
    /// The bootstrap amount for pool initialization
    pub bootstrap_amount: Uint128,
    /// Contract owner
    pub owner: String,
    /// Unbonding period in seconds
    pub unbonding_period: u64,
    /// xORO decimals
    pub xoro_decimals: u32,
    /// xORO metadata URI
    pub xoro_uri: Option<String>,
    /// Whether the contract is paused
    pub paused: bool,
}

#[cw_serde]
pub struct StateResponse {
    pub total_shares: Uint128,
    pub total_deposit: Uint128,
    pub exchange_rate: cosmwasm_std::Decimal,
}

/// This structure stores the tracking contract data (DEPRECATED: keeping for type compatibility if needed, but remove fields for safety or just keep as is)
#[cw_serde]
pub struct TrackerData {
    pub token_factory_addr: String,
}

/// The structure returned as part of set_data when staking or unstaking
#[cw_serde]
pub struct StakingResponse {
    /// The ORO denom
    pub oro_amount: Uint128,
    /// The xORO denom
    pub xoro_amount: Uint128,
}
