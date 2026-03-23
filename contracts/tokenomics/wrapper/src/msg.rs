use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

use crate::state::Config;

/// Instantiation parameters for the wrapper contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// The native/IBC/tokenfactory denom to be locked.
    /// Examples:
    ///   - "ibc/F1073E5895A25347EF10A7B50783D3048882726895AEE1176C498C45D0576F2B"
    ///   - "coin.zig13g54j2l7039e0py7fr8ekfesqzfd82hdwef3837zzaj9mptq3e7skt4xdr.oro"
    ///   - "uzig"
    pub lock_denom: String,

    /// Sub-denom suffix used when creating the ZigChain factory token.
    /// The resulting full denom will be:
    ///   "coin.<contract_address>.<mint_subdenom>"
    /// Example: "xoro"
    pub mint_subdenom: String,

    /// Optional URI for the minted token's on-chain metadata.
    pub mint_uri: Option<String>,

    /// Number of decimals for the minted token (default: 6).
    pub mint_decimals: Option<u32>,

    /// Owner / admin of the contract (can pause/unpause and update config).
    pub owner: String,
}

/// Execute messages for the wrapper contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Lock `lock_denom` tokens and receive an equal amount of `mint_denom` tokens.
    /// Send native funds with this call.
    /// If `receiver` is None the minted tokens go to the sender.
    Lock { receiver: Option<String> },

    /// Burn `mint_denom` tokens and receive an equal amount of `lock_denom` tokens back.
    /// Send mint tokens as native funds with this call.
    /// If `receiver` is None the unlocked tokens go to the sender.
    Unlock { receiver: Option<String> },

    /// Pause all lock/unlock operations. Owner only.
    Pause {},

    /// Resume lock/unlock operations. Owner only.
    Unpause {},

    /// Proposes a new owner. The proposed owner must call ClaimOwnership within expires_in seconds.
    /// Only callable by the current owner.
    ProposeNewOwner { owner: String, expires_in: u64 },

    /// Removes the pending ownership proposal. Only callable by the current owner.
    DropOwnershipProposal {},

    /// Claims a pending ownership proposal. Only callable by the proposed new owner.
    ClaimOwnership {},

    /// Recovers any `lock_denom` tokens that were sent directly to the contract without minting,
    /// restoring the 1:1 invariant between locked tokens and minted supply. Owner only.
    RecoverExcess {},
}

/// Message used to trigger a contract migration.
#[cw_serde]
pub struct MigrateMsg {}

/// Query messages for the wrapper contract.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the full contract [`Config`].
    #[returns(Config)]
    Config {},

    /// Returns the total amount of `lock_denom` currently held (locked) in
    /// the contract. NOTE: If users send `lock_denom` directly to the contract
    /// via a standard bank send, this value will encompass that excess.
    /// To check the true 1:1 backed invariant, compare this with `MintSupply`.
    #[returns(Uint128)]
    TotalLocked {},

    /// Returns the total circulating supply of the `mint_denom`.
    #[returns(Uint128)]
    MintSupply {},
}
