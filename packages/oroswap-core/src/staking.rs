//! Single-asset ORO staking: 1:1 stake registry + per-denom rewards.
//!
//! `STAKES: Map<&Addr, Uint128>` is the authoritative per-user balance and
//! `TOTAL_STAKES: Item<Uint128>` is the sum. There is no share token and no
//! exchange rate — 1 staked ORO is 1 unit of stake. Rewards are distributed
//! per-denom via a `global_index` / `user_index` accumulator using staked ORO
//! as the weight.
//!
//! Because the relationship is strictly 1:1, the classic first-depositor /
//! share-inflation attack is structurally impossible: a donation of ORO is
//! neither credited to anyone's stake nor exchanged for any claim on existing
//! stakers' principal.

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

/// Parameters used to instantiate the staking contract.
///
/// Pause authorities are intentionally NOT part of instantiate; the owner adds
/// them post-instantiation via [`ExecuteMsg::AddPauseAuthorities`], matching
/// the factory / incentives operator-flow pattern.
#[cw_serde]
pub struct InstantiateMsg {
    /// Contract owner (admin authority).
    pub owner: String,
    /// The ORO token denom that users stake.
    pub oro_denom: String,
    /// Unbonding period in seconds.
    pub unbonding_period: u64,
    /// Minimum ORO amount per Enter or partial Leave; full exits bypass this. Must be > 0.
    pub minimum_stake_amount: Uint128,
}

/// A single pending unbonding entry for a user.
#[cw_serde]
pub struct UnbondingEntry {
    /// ORO amount to be returned, fixed at Leave time.
    pub oro_amount: Uint128,
    /// Accrued reward coins to be returned alongside ORO.
    #[serde(default)]
    pub reward_coins: Vec<cosmwasm_std::Coin>,
    /// Unix timestamp (seconds) after which ORO can be claimed.
    pub unlock_time: u64,
    /// Address that will receive the funds when Claim is called.
    pub receiver: String,
}

/// Execute messages exposed by the staking contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Deposit ORO via `info.funds` and credit the sender's stake 1:1.
    Enter {},

    /// Begin unbonding `amount` ORO from the caller's stake; queues an entry
    /// for `recipient` claimable after `unbonding_period`.
    Leave {
        amount: Uint128,
        recipient: String,
    },

    /// Claim all matured unbonding entries for `info.sender`.
    Claim {},

    /// Fund a per-denom reward stream that drips over `duration` seconds.
    /// Reward denom and amount come from `info.funds`; RPS is derived on-chain
    /// as `amount / duration` so the caller cannot pass a misleading rate. Owner only.
    AddReward {
        duration: u64,
    },

    /// Emergency withdrawal of non-ORO, non-reward-stream tokens accidentally
    /// sent to the contract. Owner only.
    WithdrawExcessTokens {
        denom: String,
        recipient: Option<String>,
    },

    /// Update mutable configuration fields. Owner only.
    ///
    /// Ownership changes are intentionally NOT exposed here — use the
    /// two-step `ProposeNewOwner` + `ClaimOwnership` flow so a fat-fingered
    /// hand-off can't strand the contract behind an unreachable address.
    UpdateConfig {
        unbonding_period: Option<u64>,
        minimum_stake_amount: Option<Uint128>,
    },

    /// Pause the contract. Owner or pause-authority.
    Pause {},
    /// Unpause the contract. Owner only.
    Unpause {},

    /// Add pause-authority addresses. Authorities can `Pause` but cannot
    /// `Unpause`, so a compromised authority key cannot grief by toggling. Owner only.
    AddPauseAuthorities { addresses: Vec<String> },

    /// Remove addresses from the pause-authority set. Owner only.
    RemovePauseAuthorities { addresses: Vec<String> },

    /// Sweep accumulated orphaned rewards (emissions accrued while no users
    /// were staked) to `recipient` or the owner. Owner only.
    ClaimOrphanedRewards { recipient: Option<String> },

    /// Propose a new owner; must call `ClaimOwnership` within `expires_in`. Owner only.
    ProposeNewOwner { owner: String, expires_in: u64 },
    /// Remove the pending ownership proposal. Owner only.
    DropOwnershipProposal {},
    /// Accept a pending ownership proposal. Proposed-owner only.
    ClaimOwnership {},
}

/// Query messages exposed by the staking contract.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the full contract configuration.
    #[returns(Config)]
    Config {},

    /// Returns the current pool state.
    #[returns(StateResponse)]
    State {},

    /// Returns the stake balance held by `address`.
    #[returns(Uint128)]
    StakeOf { address: String },

    /// Returns the sum of all stake balances. Kept under the legacy
    /// `TotalShares` name for API stability; shares and ORO amounts are
    /// interchangeable.
    #[returns(Uint128)]
    TotalShares {},

    /// Returns all pending reward coins accrued for `address`.
    #[returns(Vec<cosmwasm_std::Coin>)]
    PendingRewards { address: String },

    /// Returns all pending unbonding entries for `address`.
    #[returns(Vec<UnbondingEntry>)]
    UnbondingEntries { address: String },

    /// Returns the configured unbonding period in seconds.
    #[returns(u64)]
    UnbondingPeriod {},

    /// Returns the total ORO currently locked in unbonding entries.
    #[returns(Uint128)]
    TotalLocked {},

    /// Returns the addresses currently in the pause-authority set
    /// (excluding the owner, who has implicit authority).
    #[returns(Vec<String>)]
    PauseAuthorities {},
}

/// Main parameters stored on-chain for the staking contract.
#[cw_serde]
pub struct Config {
    /// Contract owner (admin authority).
    pub owner: cosmwasm_std::Addr,
    /// The ORO token denom users stake.
    pub oro_denom: String,
    /// Unbonding period in seconds.
    pub unbonding_period: u64,
    /// Minimum ORO amount per Enter or partial Leave; full exits bypass this. Must be > 0.
    ///
    /// `#[serde(default)]` makes legacy on-chain bytes (pre-field) deserialise
    /// as `Uint128::zero()`; the `migrate` handler then rewrites a non-zero
    /// default. New instantiations reject zero up front.
    #[serde(default)]
    pub minimum_stake_amount: Uint128,
    /// Whether the contract is paused.
    ///
    /// `#[serde(default)]` is load-bearing: legacy bytes serialised before
    /// `paused` existed (pre-2.2.0) deserialise as `false`, so `CONFIG.load()`
    /// after `migrate` from such a version succeeds without the migrate
    /// handler having to rewrite Config. (Audit N-2.)
    #[serde(default)]
    pub paused: bool,
}

/// Pool state snapshot returned by [`QueryMsg::State`].
#[cw_serde]
pub struct StateResponse {
    /// Sum of all stake balances. Equal to `total_deposit` under the 1:1
    /// model; both fields preserve backwards-compatible JSON shape.
    pub total_shares: Uint128,
    /// Total ORO principal backing live stakers (excluding unbonding).
    pub total_deposit: Uint128,
}

/// Message used to trigger a contract migration.
#[cw_serde]
pub struct MigrateMsg {}

/// Returned as `set_data` from Enter/Leave. `share_amount` always equals
/// `oro_amount` under the 1:1 model; retained for backwards-compatible JSON.
#[cw_serde]
pub struct StakingResponse {
    pub oro_amount: Uint128,
    pub share_amount: Uint128,
}
