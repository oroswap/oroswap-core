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
    /// Example: "oro"
    pub mint_subdenom: String,

    /// Optional URI for the minted token's on-chain metadata.
    pub mint_uri: Option<String>,

    /// Optional hash of the content addressed by `mint_uri` (e.g. an IPFS
    /// CID or sha256 of the logo / manifest). Empty by default. Editable
    /// post-instantiate via `UpdateDenomMetadata` since v5 leaves uri_hash
    /// on the mutable side of the freeze.
    pub mint_uri_hash: Option<String>,

    /// Number of decimals for the minted token. Must match the
    /// `lock_denom`'s decimals so wallets display 1:1 wrapping correctly.
    /// Required — no default, since silently assuming 6 would mis-state
    /// precision for any non-stable source token (e.g. 18-decimal ERC-20s).
    pub mint_decimals: u32,

    /// Human-readable name for the minted token (e.g. "Wrapped ORO").
    /// Falls back to "Oroswap <subdenom>" if not provided.
    pub mint_name: Option<String>,

    /// Ticker symbol for the minted token (e.g. "xORO").
    /// Falls back to `mint_subdenom` if not provided.
    pub mint_symbol: Option<String>,

    /// Free-form description for the minted token's on-chain metadata.
    /// Falls back to "Oroswap <subdenom> Token" if not provided.
    pub mint_description: Option<String>,

    /// Owner / admin of the contract (can pause/unpause and update config).
    pub owner: String,

    /// Wrapper-enforced cap on the circulating supply of the minted denom,
    /// expressed in **base units** (i.e. already scaled by `mint_decimals`).
    ///
    /// Set this to the source asset's known maximum supply in base units —
    /// e.g. for an asset with 6 decimals and a 790,000,000-token max,
    /// `mint_max_supply = 790_000_000_000_000`. The contract validates
    /// `> 0` at instantiate; consistency with `mint_decimals` is an
    /// operator concern. The value is stored in `Config.max_supply` and
    /// is immutable after instantiate, so it must be set correctly here.
    pub mint_max_supply: Uint128,
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

    /// Pause all lock/unlock operations. Callable by the owner or any address
    /// in the pause-authority set.
    Pause {},

    /// Resume lock/unlock operations. Owner only — a compromised pause
    /// authority cannot toggle the pause state on its own.
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

    /// Update the on-chain Metadata of the minted denom. Owner only.
    /// True patch semantics: any field left as `None` keeps the value
    /// currently stored on-chain; only `Some(_)` fields are overwritten.
    /// (The tokenfactory module rewrites Metadata as a whole, so the
    /// contract keeps an authoritative copy in storage and re-encodes the
    /// full Metadata on every call — never silently regenerating defaults.)
    ///
    /// Only `description`, `uri` and `uri_hash` are accepted. The identity
    /// fields `name`, `symbol`, `display` and `denom_units` are permanently
    /// frozen by the ZIGchain v5 tokenfactory upgrade once `total_minted > 0`
    /// (which is always true after the first Lock), so this variant
    /// deliberately does not expose them. They are set once at instantiate
    /// from the InstantiateMsg fields and never mutated afterwards.
    UpdateDenomMetadata {
        description: Option<String>,
        uri: Option<String>,
        uri_hash: Option<String>,
    },

    /// Begin the two-step transfer of the minted denom's bank- and
    /// metadata-admin roles to new addresses. The new admins must follow up
    /// with a `MsgClaimDenomAdmin` on the chain themselves. Owner only.
    ///
    /// Used when retiring this wrapper: the bank_admin gates mint/burn, so
    /// the successor contract / multisig must claim before users can keep
    /// locking/unlocking.
    ProposeDenomAdmin {
        bank_admin: String,
        metadata_admin: String,
    },

    /// Add addresses with pause authority. Owner only.
    AddPauseAuthorities { authorities: Vec<String> },

    /// Remove addresses from the pause-authority set. Owner only.
    RemovePauseAuthorities { authorities: Vec<String> },
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

    /// Returns the list of addresses currently in the pause-authority set.
    /// The owner always has implicit pause authority and is NOT included.
    #[returns(Vec<String>)]
    PauseAuthorities {},
}
