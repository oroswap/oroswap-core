#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, Reply, Response, StdError, StdResult, Storage, SubMsg, Uint128,
};
use cw2::set_contract_version;
use cw_utils::must_pay;
use oroswap::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};

use oroswap::token_factory::{
    tf_burn_msg, tf_create_denom_msg_with_cap, tf_mint_msg, MsgCreateDenomResponse,
};
#[cfg(feature = "zigchain")]
use oroswap::token_factory::{tf_propose_denom_admin_msg, tf_set_denom_metadata_full_msg};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, DenomMetadata, CONFIG, DENOM_METADATA, OWNERSHIP_PROPOSAL, PAUSE_AUTHORITIES,
};

/// Contract name used for migration / cw2 version tracking.
pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
/// Contract version used for migration / cw2 version tracking.
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Reply ID emitted when the ZigChain factory `MsgCreateDenom` sub-message
/// completes. We use the reply to read back the full denom string that the
/// chain assigned to our new token.
enum ReplyIds {
    InstantiateDenom = 1,
}

impl TryFrom<u64> for ReplyIds {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(ReplyIds::InstantiateDenom),
            _ => Err(ContractError::FailedToParseReply {}),
        }
    }
}

/// Instantiate the wrapper contract.
///
/// Validates the `lock_denom` exists on-chain (non-zero supply), saves a
/// preliminary [`Config`] with an empty `mint_denom`, and fires a
/// `tf_create_denom_msg` sub-message whose reply will populate `mint_denom`.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate owner address
    let owner = deps.api.addr_validate(&msg.owner)?;

    // The `lock_denom` can be any native denom (ibc/..., coin.xxx.yyy, uzig …).
    // We do a loose existence check — not all denoms have a supply (e.g. newly
    // created IBC ones might be 0), so we only warn but never hard-fail here.
    // Instead, the real guard is that a `must_pay` in execute_lock will fail if
    // the user doesn't actually hold/send the correct denom.
    //
    // However, for fully-local tokenfactory tokens and standard native tokens we
    // CAN check supply, so we skip the check entirely to stay compatible with IBC
    // denoms whose supply may be 0 on this chain before any packet arrives.

    // Reject obviously invalid denoms: empty string
    ensure!(
        !msg.lock_denom.is_empty(),
        StdError::generic_err("lock_denom must not be empty")
    );
    ensure!(
        !msg.mint_subdenom.is_empty(),
        StdError::generic_err("mint_subdenom must not be empty")
    );

    // mint_max_supply defines the wrapper's circulating supply cap and
    // must be greater than zero. A zero value would make every subsequent
    // lock fail, with no on-chain path to correct it (the field is
    // immutable after instantiate).
    if msg.mint_max_supply.is_zero() {
        return Err(ContractError::InvalidMintMaxSupply {});
    }

    CONFIG.save(
        deps.storage,
        &Config {
            lock_denom: msg.lock_denom.clone(),
            mint_denom: "".to_string(), // populated in reply
            mint_decimals: msg.mint_decimals,
            owner: owner.clone(),
            paused: false,
            max_supply: msg.mint_max_supply,
        },
    )?;

    // Resolve the canonical metadata once, here at instantiate, so the reply
    // handler and any future `UpdateDenomMetadata` patches read the same
    // baseline. Defaults derive from `mint_subdenom`; explicit overrides win.
    let symbol = msg.mint_symbol.unwrap_or_else(|| msg.mint_subdenom.clone());
    let name = msg.mint_name.unwrap_or_else(|| format!("Oroswap {}", symbol));
    let description = msg
        .mint_description
        .unwrap_or_else(|| format!("Oroswap {} Token", symbol));
    // Pre-validate uri_hash so a malformed input fails the tx at instantiate
    // instead of in the create-denom reply, where the chain's "not a valid
    // SHA256 hash string of 64 alpha numeric characters" error is buried
    // inside a sub-msg revert.
    validate_uri_hash(&msg.mint_uri_hash)?;
    DENOM_METADATA.save(
        deps.storage,
        &DenomMetadata {
            name,
            symbol,
            description,
            uri: msg.mint_uri,
            uri_hash: msg.mint_uri_hash,
        },
    )?;

    // Emit ZigChain factory create-denom sub-message.
    //
    // The on-chain `minting_cap` is set to `Uint128::MAX` (≈ 3.4 × 10³⁸).
    // ZigChain's factory module enforces this value against the cumulative
    // `total_minted` counter, which is not decremented by burns; coupling
    // that counter to the source asset's max supply would make the cap
    // track lifetime throughput rather than peak circulating supply, and
    // the field is immutable after create-denom. Setting `Uint128::MAX`
    // keeps the chain-side check from becoming the binding constraint
    // under normal wrap/unwrap activity; the operationally meaningful cap
    // is `config.max_supply`, enforced in `execute_lock` against the live
    // circulating supply queried from the bank module.
    let create_denom_msg = SubMsg::reply_on_success(
        tf_create_denom_msg_with_cap(
            env.contract.address.to_string(),
            msg.mint_subdenom.clone(),
            Uint128::MAX,
        ),
        ReplyIds::InstantiateDenom as u64,
    );

    Ok(Response::new()
        .add_submessage(create_denom_msg)
        .add_attributes(vec![
            attr("action", "instantiate"),
            attr("contract", CONTRACT_NAME),
            attr("owner", owner),
            attr("lock_denom", msg.lock_denom),
            attr("mint_subdenom", msg.mint_subdenom),
            attr("mint_max_supply", msg.mint_max_supply),
        ]))
}

/// Handle the reply from the ZigChain factory `MsgCreateDenom`.
///
/// Reads the newly-assigned full denom back from the chain response,
/// stores it in `Config.mint_denom`, and (on ZigChain) sets token metadata
/// derived from the caller-supplied name/symbol/description (with sensible
/// defaults if the caller omitted them).
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match ReplyIds::try_from(msg.id)? {
        ReplyIds::InstantiateDenom => {
            if let cosmwasm_std::SubMsgResult::Ok(cosmwasm_std::SubMsgResponse {
                data: Some(b),
                ..
            }) = msg.result
            {
                #[cfg(not(feature = "zigchain"))]
                let MsgCreateDenomResponse { new_token_denom } = b.try_into()?;
                #[cfg(feature = "zigchain")]
                let MsgCreateDenomResponse {
                    denom: new_token_denom,
                    ..
                } = b.try_into()?;

                // FIND-013: protobuf decoding can succeed while leaving the
                // denom string empty (mismatched feature flags, future schema
                // drift). Refuse to persist an unusable mint_denom — every
                // subsequent Lock/Unlock would fail on the empty denom.
                if new_token_denom.is_empty() {
                    return Err(ContractError::FailedToParseReply {});
                }

                let mut config = CONFIG.load(deps.storage)?;
                config.mint_denom = new_token_denom.clone();
                CONFIG.save(deps.storage, &config)?;

                // On ZigChain, also set human-readable token metadata.
                // Defaults preserve the legacy "Oroswap <subdenom>" naming
                // when the caller didn't supply overrides.
                #[cfg(feature = "zigchain")]
                let set_meta_msg: Option<CosmosMsg> = {
                    let meta = DENOM_METADATA.load(deps.storage)?;
                    Some(tf_set_denom_metadata_full_msg(
                        env.contract.address.clone(),
                        new_token_denom.clone(),
                        meta.name,
                        meta.symbol.clone(),
                        meta.description,
                        meta.symbol.to_lowercase(),
                        config.mint_decimals,
                        meta.uri,
                        meta.uri_hash,
                    ))
                };
                #[cfg(not(feature = "zigchain"))]
                let set_meta_msg: Option<CosmosMsg> = None;

                let mut resp = Response::new()
                    .add_attribute("mint_denom", &new_token_denom);

                if let Some(m) = set_meta_msg {
                    resp = resp.add_message(m);
                }

                Ok(resp)
            } else {
                Err(ContractError::FailedToParseReply {})
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Execute
// ---------------------------------------------------------------------------

/// Route incoming execute messages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Lock { receiver } => {
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            deps.api.addr_validate(&recipient)?;
            execute_lock(deps, env, info, recipient)
        }
        ExecuteMsg::Unlock { receiver } => {
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            deps.api.addr_validate(&recipient)?;
            execute_unlock(deps, env, info, recipient)
        }
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Unpause {} => execute_unpause(deps, info),
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;
            propose_new_owner(deps, info, env, owner, expires_in, config.owner, OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config = CONFIG.load(deps.storage)?;
            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.owner = new_owner;
                    Ok(v)
                })?;
                Ok(())
            })
            .map_err(Into::into)
        }
        ExecuteMsg::RecoverExcess {} => execute_recover_excess(deps, env, info),
        ExecuteMsg::UpdateDenomMetadata {
            description,
            uri,
            uri_hash,
        } => execute_update_denom_metadata(deps, env, info, description, uri, uri_hash),
        ExecuteMsg::ProposeDenomAdmin {
            bank_admin,
            metadata_admin,
        } => execute_propose_denom_admin(deps, env, info, bank_admin, metadata_admin),
        ExecuteMsg::AddPauseAuthorities { authorities } => {
            execute_add_pause_authorities(deps, info, authorities)
        }
        ExecuteMsg::RemovePauseAuthorities { authorities } => {
            execute_remove_pause_authorities(deps, info, authorities)
        }
    }
}

/// Lock `lock_denom` tokens → mint equal `mint_denom` tokens to `recipient`.
///
/// Invariant: after this call,
///   `contract balance of lock_denom` increases by `amount`
///   `mint_denom total supply`          increases by `amount`
fn execute_lock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    // Exactly one coin of the correct lock_denom must be attached
    let amount = must_pay(&info, &config.lock_denom)?;

    if amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Cap on circulating supply, not cumulative mints. `query_supply` is
    // the chain's authoritative count of the wrapped denom in circulation
    // right now; burns decrement it, so the cap tracks peak supply rather
    // than lifetime throughput.
    let current_supply = deps.querier.query_supply(&config.mint_denom)?.amount;
    let new_supply = current_supply
        .checked_add(amount)
        .map_err(|e| ContractError::Std(cosmwasm_std::StdError::from(e)))?;
    if new_supply > config.max_supply {
        return Err(ContractError::MaxSupplyExceeded {
            current: current_supply,
            mint: amount,
            cap: config.max_supply,
        });
    }

    // Mint 1:1 using ZigChain factory
    let mint_coin = coin(amount.u128(), &config.mint_denom);
    let mint_msgs: Vec<CosmosMsg> =
        tf_mint_msg(env.contract.address, mint_coin, recipient.clone());

    Ok(Response::new()
        .add_messages(mint_msgs)
        .add_attributes([
            attr("action", "lock"),
            attr("sender", &info.sender),
            attr("recipient", &recipient),
            attr("lock_denom", &config.lock_denom),
            attr("mint_denom", &config.mint_denom),
            attr("amount", amount),
        ]))
}

/// Unlock: burn `mint_denom` tokens → return equal `lock_denom` tokens to `recipient`.
///
/// Invariant: after this call,
///   `contract balance of lock_denom` decreases by `amount`
///   `mint_denom total supply`          decreases by `amount`
fn execute_unlock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    // Exactly one coin of the correct mint_denom must be attached
    let amount = must_pay(&info, &config.mint_denom)?;

    if amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Burn the minted tokens
    let burn_msg: CosmosMsg =
        tf_burn_msg(env.contract.address, coin(amount.u128(), &config.mint_denom));

    // Return the locked tokens to recipient
    let send_msg: CosmosMsg = BankMsg::Send {
        to_address: recipient.clone(),
        amount: vec![Coin {
            denom: config.lock_denom.clone(),
            amount,
        }],
    }
    .into();

    Ok(Response::new()
        .add_message(burn_msg)
        .add_message(send_msg)
        .add_attributes([
            attr("action", "unlock"),
            attr("sender", &info.sender),
            attr("recipient", &recipient),
            attr("mint_denom", &config.mint_denom),
            attr("lock_denom", &config.lock_denom),
            attr("amount", amount),
        ]))
}

/// Pause the contract. Callable by the owner or any address registered in
/// `PAUSE_AUTHORITIES`.
fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    check_pause_authority(deps.storage, &info.sender)?;
    let mut config = CONFIG.load(deps.storage)?;
    config.paused = true;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes([
        attr("action", "pause"),
        attr("sender", info.sender),
    ]))
}

/// Unpause the contract. Owner only — keeping unpause to the owner means a
/// compromised pause authority can never grief by toggling pause state.
fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    check_unpause_authority(deps.storage, &info.sender)?;
    let mut config = CONFIG.load(deps.storage)?;
    config.paused = false;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes([
        attr("action", "unpause"),
        attr("sender", info.sender),
    ]))
}

/// Returns an error if the contract is paused.
fn check_paused(config: &Config) -> Result<(), ContractError> {
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    Ok(())
}

/// Validate the shape of a uri_hash exactly as the ZIGchain factory module
/// does at message-validation time: must be empty (treat as None) OR exactly
/// 64 ASCII hex characters (a valid SHA256 hex digest). Surfaces the typed
/// `InvalidUriHash` error before the tx hits the chain — far better operator
/// UX than the buried "valid SHA256 hash string of 64 alpha numeric
/// characters" error returned from inside a sub-msg revert.
fn validate_uri_hash(uri_hash: &Option<String>) -> Result<(), ContractError> {
    if let Some(h) = uri_hash {
        if h.is_empty() {
            return Ok(());
        }
        if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ContractError::InvalidUriHash { got: h.len() });
        }
    }
    Ok(())
}

/// Pause is allowed if the sender is the contract owner or sits in the
/// pause-authority set.
fn check_pause_authority(storage: &dyn Storage, sender: &Addr) -> Result<(), ContractError> {
    let config = CONFIG.load(storage)?;

    if sender == &config.owner {
        return Ok(());
    }

    if PAUSE_AUTHORITIES.has(storage, sender) {
        return Ok(());
    }

    Err(ContractError::NoPauseAuthority {})
}

/// Unpause is restricted to the owner.
fn check_unpause_authority(storage: &dyn Storage, sender: &Addr) -> Result<(), ContractError> {
    let config = CONFIG.load(storage)?;

    if sender == &config.owner {
        return Ok(());
    }

    Err(ContractError::NoUnpauseAuthority {})
}

/// Recovers tokens sent directly to the contract. Owner only.
fn execute_recover_excess(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let balance: Uint128 = deps
        .querier
        .query_balance(&env.contract.address, &config.lock_denom)?
        .amount;
    let supply: Uint128 = deps.querier.query_supply(&config.mint_denom)?.amount;

    if balance <= supply {
        return Err(StdError::generic_err("No excess tokens to recover").into());
    }

    let excess = balance - supply;
    let send_msg: CosmosMsg = BankMsg::Send {
        to_address: config.owner.to_string(),
        amount: vec![coin(excess.u128(), &config.lock_denom)],
    }
    .into();

    Ok(Response::new()
        .add_message(send_msg)
        .add_attributes([
            attr("action", "recover_excess"),
            attr("amount", excess),
        ]))
}

/// Update the on-chain Metadata for the minted denom. Owner only.
///
/// The tokenfactory module replaces Metadata wholesale on each Set call,
/// so callers should pass the values they want for every field that should
/// change; any field left as `None` keeps its current value… in the *contract's*
/// view of metadata. We do NOT read the chain's current Metadata here (no
/// stargate query helper is available in this contract), so a `None` field
/// means "use the value previously persisted by this contract", and on first
/// update after instantiate we fall back to the same defaults the reply
/// handler uses.
/// Update the mutable subset of the minted denom's on-chain Metadata.
///
/// Only `description`, `uri` and `uri_hash` are accepted. The identity fields
/// (`name`, `symbol`, `display`, `denom_units`) are permanently frozen by the
/// ZIGchain v5 tokenfactory upgrade once `total_minted > 0` — which is always
/// true after the first Lock — so any update touching them would be rejected
/// by the chain. They are set once at instantiate from `InstantiateMsg`
fn execute_update_denom_metadata(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    description: Option<String>,
    uri: Option<String>,
    uri_hash: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if config.mint_denom.is_empty() {
        return Err(StdError::generic_err(
            "mint_denom not yet initialised — reply has not completed",
        )
        .into());
    }

    #[cfg(feature = "zigchain")]
    {
        // Pre-validate uri_hash (see InstantiateMsg path) so chain-side
        // shape rejection is surfaced as a typed contract error.
        validate_uri_hash(&uri_hash)?;

        // Patch only the editable fields. name / symbol / display / decimals
        // are read back as-is from the stored copy so the full-set msg is
        // identity-preserving on the frozen fields.
        let mut meta = DENOM_METADATA.load(deps.storage)?;
        if let Some(v) = description {
            meta.description = v;
        }
        if let Some(v) = uri {
            meta.uri = Some(v);
        }
        if let Some(v) = uri_hash {
            meta.uri_hash = Some(v);
        }
        DENOM_METADATA.save(deps.storage, &meta)?;

        let msg = tf_set_denom_metadata_full_msg(
            env.contract.address.clone(),
            config.mint_denom.clone(),
            meta.name.clone(),           // frozen post-v5 — re-sent unchanged
            meta.symbol.clone(),         // frozen post-v5 — re-sent unchanged
            meta.description.clone(),    // editable
            meta.symbol.to_lowercase(),  // display, frozen — derived, unchanged
            config.mint_decimals,        // denom_units exponent, frozen
            meta.uri.clone(),            // editable
            meta.uri_hash.clone(),       // editable
        );

        // cosmwasm-std rejects empty attribute values, so only attach the
        // editable fields that actually carry a value after the patch.
        let mut attrs = vec![attr("action", "update_denom_metadata")];
        if !meta.description.is_empty() {
            attrs.push(attr("description", meta.description));
        }
        if let Some(u) = meta.uri {
            if !u.is_empty() {
                attrs.push(attr("uri", u));
            }
        }
        if let Some(h) = meta.uri_hash {
            if !h.is_empty() {
                attrs.push(attr("uri_hash", h));
            }
        }
        Ok(Response::new().add_message(msg).add_attributes(attrs))
    }

    #[cfg(not(feature = "zigchain"))]
    {
        let _ = (env, description, uri, uri_hash);
        Err(StdError::generic_err(
            "UpdateDenomMetadata is only supported with the zigchain feature",
        )
        .into())
    }
}

/// Begin the two-step transfer of the minted denom's bank- and metadata-admin
/// roles. The proposed admins must follow up with `MsgClaimDenomAdmin`
/// themselves before the transfer takes effect on chain. Owner only.
fn execute_propose_denom_admin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bank_admin: String,
    metadata_admin: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if config.mint_denom.is_empty() {
        return Err(StdError::generic_err(
            "mint_denom not yet initialised — reply has not completed",
        )
        .into());
    }

    // Validate addresses up-front so a typo can't strand the denom.
    deps.api.addr_validate(&bank_admin)?;
    deps.api.addr_validate(&metadata_admin)?;

    #[cfg(feature = "zigchain")]
    {
        let msg = tf_propose_denom_admin_msg(
            env.contract.address.clone(),
            config.mint_denom.clone(),
            bank_admin.clone(),
            metadata_admin.clone(),
        );

        Ok(Response::new()
            .add_message(msg)
            .add_attributes([
                attr("action", "propose_denom_admin"),
                attr("denom", config.mint_denom),
                attr("bank_admin", bank_admin),
                attr("metadata_admin", metadata_admin),
            ]))
    }

    #[cfg(not(feature = "zigchain"))]
    {
        let _ = (env, bank_admin, metadata_admin);
        Err(StdError::generic_err(
            "ProposeDenomAdmin is only supported with the zigchain feature",
        )
        .into())
    }
}

/// Add addresses to the pause-authority set. Owner only.
fn execute_add_pause_authorities(
    deps: DepsMut,
    info: MessageInfo,
    authorities: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut added = 0u32;
    for raw in authorities {
        let addr = deps
            .api
            .addr_validate(&raw)
            .map_err(|_| ContractError::InvalidPauseAuthority { address: raw.clone() })?;
        if !PAUSE_AUTHORITIES.has(deps.storage, &addr) {
            PAUSE_AUTHORITIES.save(deps.storage, &addr, &())?;
            added += 1;
        }
    }

    Ok(Response::new().add_attributes([
        attr("action", "add_pause_authorities"),
        attr("added_count", added.to_string()),
    ]))
}

/// Remove addresses from the pause-authority set. Owner only.
fn execute_remove_pause_authorities(
    deps: DepsMut,
    info: MessageInfo,
    authorities: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut removed = 0u32;
    for raw in authorities {
        let addr = deps
            .api
            .addr_validate(&raw)
            .map_err(|_| ContractError::InvalidPauseAuthority { address: raw.clone() })?;
        if PAUSE_AUTHORITIES.has(deps.storage, &addr) {
            PAUSE_AUTHORITIES.remove(deps.storage, &addr);
            removed += 1;
        }
    }

    Ok(Response::new().add_attributes([
        attr("action", "remove_pause_authorities"),
        attr("removed_count", removed.to_string()),
    ]))
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Route incoming query messages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::TotalLocked {} => {
            let config = CONFIG.load(deps.storage)?;
            // The total locked amount equals the contract's own balance of lock_denom
            let balance: Uint128 = deps
                .querier
                .query_balance(&env.contract.address, &config.lock_denom)?
                .amount;
            to_json_binary(&balance)
        }
        QueryMsg::MintSupply {} => {
            let config = CONFIG.load(deps.storage)?;
            let supply: Uint128 = deps.querier.query_supply(&config.mint_denom)?.amount;
            to_json_binary(&supply)
        }
        QueryMsg::PauseAuthorities {} => to_json_binary(&query_pause_authorities(deps)?),
    }
}

fn query_pause_authorities(deps: Deps) -> StdResult<Vec<String>> {
    PAUSE_AUTHORITIES
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| item.map(|(addr, _)| addr.to_string()))
        .collect()
}
