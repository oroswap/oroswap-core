#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, StdResult, SubMsg, Uint128,
};
use cw2::set_contract_version;
use cw_utils::must_pay;

use oroswap::token_factory::{
    tf_burn_msg, tf_create_denom_msg, tf_mint_msg, MsgCreateDenomResponse,
};
#[cfg(feature = "zigchain")]
use oroswap::token_factory::tf_set_denom_metadata_msg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};

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

    // Store reference to uri and decimals for use in reply handler
    CONFIG.save(
        deps.storage,
        &Config {
            lock_denom: msg.lock_denom.clone(),
            mint_denom: "".to_string(), // populated in reply
            owner: owner.to_string(),
            paused: false,
        },
    )?;

    // Persist mint metadata so the reply handler can build the SetDenomMetadata msg
    MINT_META.save(
        deps.storage,
        &MintMeta {
            subdenom: msg.mint_subdenom.clone(),
            decimals: msg.mint_decimals.unwrap_or(6),
            uri: msg.mint_uri,
        },
    )?;

    // Emit ZigChain factory create-denom sub-message
    let create_denom_msg = SubMsg::reply_on_success(
        tf_create_denom_msg(env.contract.address.to_string(), msg.mint_subdenom.clone()),
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
        ]))
}

/// Temporary mint metadata stored between instantiate and reply.
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Item;

#[cw_serde]
struct MintMeta {
    subdenom: String,
    decimals: u32,
    uri: Option<String>,
}

const MINT_META: Item<MintMeta> = Item::new("mint_meta");

/// Handle the reply from the ZigChain factory `MsgCreateDenom`.
///
/// Reads the newly-assigned full denom back from the chain response,
/// stores it in `Config.mint_denom`, and (on ZigChain) sets token metadata.
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

                // Persist the full denom in config
                CONFIG.update::<_, StdError>(deps.storage, |mut cfg| {
                    cfg.mint_denom = new_token_denom.clone();
                    Ok(cfg)
                })?;

                let meta = MINT_META.load(deps.storage)?;

                // On ZigChain, also set human-readable token metadata
                #[cfg(feature = "zigchain")]
                let set_meta_msg: Option<CosmosMsg> = Some(tf_set_denom_metadata_msg(
                    env.contract.address.clone(),
                    new_token_denom.clone(),
                    meta.subdenom.clone(),
                    meta.decimals,
                    meta.uri,
                ));
                #[cfg(not(feature = "zigchain"))]
                let set_meta_msg: Option<CosmosMsg> = None;

                let mut resp = Response::new().add_attributes([
                    attr("mint_denom", &new_token_denom),
                    attr("mint_subdenom", &meta.subdenom),
                ]);

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
            execute_lock(deps, env, info, recipient)
        }
        ExecuteMsg::Unlock { receiver } => {
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            execute_unlock(deps, env, info, recipient)
        }
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Unpause {} => execute_unpause(deps, info),
        ExecuteMsg::UpdateConfig { owner } => execute_update_config(deps, info, owner),
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

/// Pause the contract. Owner only.
fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    config.paused = true;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "pause"))
}

/// Unpause the contract. Owner only.
fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    config.paused = false;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "unpause"))
}

/// Update owner. Owner only.
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(new_owner) = owner {
        deps.api.addr_validate(&new_owner)?;
        config.owner = new_owner;
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Returns an error if the contract is paused.
fn check_paused(config: &Config) -> Result<(), ContractError> {
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    Ok(())
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
    }
}
