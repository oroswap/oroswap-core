use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, Uint128};

/// This structure describes the parameters used for creating a contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// The ORO token contract address
    pub deposit_token_denom: String,
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
    /// Accrued reward tokens to be returned alongside ORO.
    #[serde(default)]
    pub reward_coins: Vec<cosmwasm_std::Coin>,
    /// Unix timestamp (in seconds) after which ORO can be claimed.
    pub unlock_time: u64,
    /// The address that will receive the funds when Claim is called.
    pub receiver: String,
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
    /// The receiver is locked in at this point and stored in the unbonding entry.
    /// If not set, the sender will receive ORO on Claim.
    Leave { receiver: Option<String> },
    /// Claims all matured unbonding entries for the sender.
    /// Funds go to the receiver set during Leave.
    Claim {},
    /// Adds reward tokens to the contract to be distributed to stakers.
    /// Admin only.
    AddReward {
        /// Number of seconds over which rewards are distributed.
        duration: u64,
    },
    /// Updates the contract configuration. Only callable by the owner.
    UpdateConfig {
        unbonding_period: Option<u64>,
    },
    /// Proposes a new owner. The proposed owner must call ClaimOwnership within expires_in seconds.
    /// Only callable by the current owner.
    ProposeNewOwner {
        owner: String,
        expires_in: u64,
    },
    /// Removes the pending ownership proposal. Only callable by the current owner.
    DropOwnershipProposal {},
    /// Claims a pending ownership proposal. Only callable by the proposed new owner.
    ClaimOwnership {},
    /// Pauses the contract. Only callable by the owner.
    Pause {},
    /// Unpauses the contract. Only callable by the owner.
    Unpause {},
    /// Emergency withdrawal of non-staked tokens accidentally sent to the contract.
    /// For ORO: only withdraws the excess above (effective_deposit + total_locked + reward_oro_reserve).
    /// For other denoms: withdraws the full balance.
    /// Only callable by the owner.
    WithdrawExcessTokens { denom: String, recipient: Option<String> },

    /// Add addresses authorised to pause the contract (in addition to the
    /// owner). Owner only. Authorities can `Pause` but cannot `Unpause`, so
    /// a compromised authority key can never grief by toggling.
    AddPauseAuthorities { authorities: Vec<String> },

    /// Remove addresses from the pause-authority set. Owner only.
    RemovePauseAuthorities { authorities: Vec<String> },
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
    /// Returns all pending rewards for the given address.
    #[returns(Vec<cosmwasm_std::Coin>)]
    PendingRewards { address: String },

    /// Returns the addresses currently in the pause-authority set.
    /// The owner has implicit pause authority and is NOT included.
    #[returns(Vec<String>)]
    PauseAuthorities {},
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
    /// Whether the contract is paused.
    ///
    /// `#[serde(default)]` is load-bearing: any legacy on-chain bytes
    /// serialised before `paused` was added (pre-2.2.0) deserialise as
    /// `false`, so a `CONFIG.load()` after `migrate` from such a version
    /// succeeds without the migrate handler having to rewrite Config. The
    /// matching default-on-deserialisation behaviour is what makes the
    /// migration path lazy and safe. (Audit N-2.)
    #[serde(default)]
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

/// Message used to trigger a contract migration.
#[cw_serde]
pub struct MigrateMsg {}

/// The structure returned as part of set_data when staking or unstaking
#[cw_serde]
pub struct StakingResponse {
    /// The ORO denom
    pub oro_amount: Uint128,
    /// The xORO denom
    pub xoro_amount: Uint128,
}
