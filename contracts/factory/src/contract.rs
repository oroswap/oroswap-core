use std::collections::HashSet;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Reply,
    ReplyOn, Response, StdError, StdResult, SubMsg, SubMsgResponse, SubMsgResult, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw_utils::parse_instantiate_response_data;
use itertools::Itertools;

use oroswap::asset::{addr_opt_validate, AssetInfo, PairInfo};
use oroswap::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};
use oroswap::factory::{
    Config, ConfigResponse, ExecuteMsg, FeeInfoResponse, InstantiateMsg, MigrateMsg, PairConfig,
    PairType, PairsResponse, QueryMsg, StartAfter, TrackerConfig,
};
use oroswap::incentives::ExecuteMsg::DeactivatePool;
use oroswap::pair::InstantiateMsg as PairInstantiateMsg;

use crate::error::ContractError;
use crate::querier::query_pair_info;
use crate::state::{
    check_asset_infos, pair_key, read_pairs, TmpPairInfo, CONFIG, OWNERSHIP_PROPOSAL, PAIRS,
    PAIR_CONFIGS, TMP_PAIR_INFO, TRACKER_CONFIG, PAUSED_PAIRS, PAUSE_AUTHORITIES,
};

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "oroswap-factory";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// A `reply` call code ID used in a sub-message.
const INSTANTIATE_PAIR_REPLY_ID: u64 = 1;

/// Creates a new contract with the specified parameters packed in the `msg` variable.
///
/// * **msg**  is message which contains the parameters used for creating the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        token_code_id: msg.token_code_id,
        fee_address: None,
        generator_address: None,
        whitelist_code_id: msg.whitelist_code_id,
        coin_registry_address: deps.api.addr_validate(&msg.coin_registry_address)?,
    };

    config.generator_address = addr_opt_validate(deps.api, &msg.generator_address)?;

    config.fee_address = addr_opt_validate(deps.api, &msg.fee_address)?;

    let config_set: HashSet<String> = msg
        .pair_configs
        .iter()
        .map(|pc| pc.pair_type.to_string())
        .collect();

    if config_set.len() != msg.pair_configs.len() {
        return Err(ContractError::PairConfigDuplicate {});
    }

    for pc in msg.pair_configs.iter() {
        // Validate total and maker fee bps
        if !pc.valid_fee_bps() {
            return Err(ContractError::PairConfigInvalidFeeBps {});
        }
        PAIR_CONFIGS.save(deps.storage, pc.pair_type.to_string(), pc)?;
    }
    CONFIG.save(deps.storage, &config)?;

    if let Some(tracker_config) = msg.tracker_config {
        TRACKER_CONFIG.save(
            deps.storage,
            &TrackerConfig {
                code_id: tracker_config.code_id,
                token_factory_addr: deps
                    .api
                    .addr_validate(&tracker_config.token_factory_addr)?
                    .to_string(),
            },
        )?;
    }

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "instantiate"),
            attr("contract", CONTRACT_NAME),
        ]))
}

/// Data structure used to update general contract parameters.
pub struct UpdateConfig {
    /// This is the CW20 token contract code identifier
    token_code_id: Option<u64>,
    /// Contract address to send governance fees to (the Maker)
    fee_address: Option<String>,
    /// Generator contract address
    generator_address: Option<String>,
    /// CW1 whitelist contract code id used to store 3rd party staking rewards
    whitelist_code_id: Option<u64>,
    coin_registry_address: Option<String>,
}

/// Exposes all the execute functions available in the contract.
/// * **msg** is an object of type [`ExecuteMsg`].
///
/// ## Variants
/// * **ExecuteMsg::UpdateConfig {
///             token_code_id,
///             fee_address,
///             generator_address,
///         }** Updates general contract parameters.
///
/// * **ExecuteMsg::UpdatePairConfig { config }** Updates a pair type
/// * configuration or creates a new pair type if a [`Custom`] name is used (which hasn't been used before).
///
/// * **ExecuteMsg::CreatePair {
///             pair_type,
///             asset_infos,
///             init_params,
///         }** Creates a new pair with the specified input parameters.
///
/// * **ExecuteMsg::Deregister { asset_infos }** Removes an existing pair from the factory.
/// * The asset information is for the assets that are traded in the pair.
///
/// * **ExecuteMsg::ProposeNewOwner { owner, expires_in }** Creates a request to change contract ownership.
///
/// * **ExecuteMsg::DropOwnershipProposal {}** Removes a request to change contract ownership.
///
/// * **ExecuteMsg::ClaimOwnership {}** Claims contract ownership.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            token_code_id,
            fee_address,
            generator_address,
            whitelist_code_id,
            coin_registry_address,
        } => execute_update_config(
            deps,
            info,
            UpdateConfig {
                token_code_id,
                fee_address,
                generator_address,
                whitelist_code_id,
                coin_registry_address,
            },
        ),
        ExecuteMsg::UpdatePairConfig { config } => execute_update_pair_config(deps, info, config),
        ExecuteMsg::CreatePair {
            pair_type,
            asset_infos,
            init_params,
        } => execute_create_pair(deps, info, env, pair_type, asset_infos, init_params),
        ExecuteMsg::Deregister { asset_infos, pair_type } => deregister(deps, info, asset_infos, pair_type),
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;

            propose_new_owner(
                deps,
                info,
                env,
                owner,
                expires_in,
                config.owner,
                OWNERSHIP_PROPOSAL,
            )
            .map_err(Into::into)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config = CONFIG.load(deps.storage)?;

            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG
                    .update::<_, StdError>(deps.storage, |mut v| {
                        v.owner = new_owner;
                        Ok(v)
                    })
                    .map(|_| ())
            })
            .map_err(Into::into)
        }
        ExecuteMsg::UpdateTrackerConfig {
            tracker_code_id,
            token_factory_addr,
        } => update_tracker_config(deps, info, tracker_code_id, token_factory_addr),
        ExecuteMsg::PausePair { asset_infos, pair_type } => pause_pair(deps, info, asset_infos, pair_type),
        ExecuteMsg::UnpausePair { asset_infos, pair_type } => unpause_pair(deps, info, asset_infos, pair_type),
        ExecuteMsg::PausePairsBatch { batch_size } => pause_pairs_batch(deps, info, batch_size),
        ExecuteMsg::UnpausePairsBatch { batch_size } => unpause_pairs_batch(deps, info, batch_size),
        ExecuteMsg::AddPauseAuthorities { authorities } => add_pause_authorities(deps, info, authorities),
        ExecuteMsg::RemovePauseAuthorities { authorities } => remove_pause_authorities(deps, info, authorities),
    }
}

/// Updates general contract settings.
///
/// * **param** is an object of type [`UpdateConfig`] that contains the parameters to update.
///
/// ## Executor
/// Only the owner can execute this.
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    param: UpdateConfig,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(fee_address) = param.fee_address {
        // Validate address format
        config.fee_address = Some(deps.api.addr_validate(&fee_address)?);
    }

    if let Some(generator_address) = param.generator_address {
        // Validate the address format
        config.generator_address = Some(deps.api.addr_validate(&generator_address)?);
    }

    if let Some(token_code_id) = param.token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(code_id) = param.whitelist_code_id {
        config.whitelist_code_id = code_id;
    }

    if let Some(coin_registry_address) = param.coin_registry_address {
        config.coin_registry_address = deps.api.addr_validate(&coin_registry_address)?;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Updates a pair type's configuration.
///
/// * **pair_config** is an object of type [`PairConfig`] that contains the pair type information to update.
///
/// ## Executor
/// Only the owner can execute this.
pub fn execute_update_pair_config(
    deps: DepsMut,
    info: MessageInfo,
    pair_config: PairConfig,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Validate total and maker fee bps
    if !pair_config.valid_fee_bps() {
        return Err(ContractError::PairConfigInvalidFeeBps {});
    }

    PAIR_CONFIGS.save(
        deps.storage,
        pair_config.pair_type.to_string(),
        &pair_config,
    )?;

    Ok(Response::new().add_attribute("action", "update_pair_config"))
}

/// Creates a new pair of `pair_type` with the assets specified in `asset_infos`.
///
/// * **pair_type** is the pair type of the newly created pair.
///
/// * **asset_infos** is a vector with assets for which we create a pair.
///
/// * **init_params** These are packed params used for custom pair types that need extra data to be instantiated.
pub fn execute_create_pair(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    pair_type: PairType,
    asset_infos: Vec<AssetInfo>,
    init_params: Option<Binary>,
) -> Result<Response, ContractError> {
    check_asset_infos(deps.api, &asset_infos)?;

    let config = CONFIG.load(deps.storage)?;

    if PAIRS.has(deps.storage, &pair_key(&asset_infos, &pair_type)) {
        return Err(ContractError::PairWasCreated {});
    }

    // Get pair type from config
    let pair_config = PAIR_CONFIGS
        .load(deps.storage, pair_type.to_string())
        .map_err(|_| ContractError::PairConfigNotFound {})?;

    if pair_config.permissioned && info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Check if pair config is disabled
    if pair_config.is_disabled {
        return Err(ContractError::PairConfigDisabled {});
    }

    // Check if pool creation fee is included
    let pool_creation_fee = pair_config.pool_creation_fee;
    let funds = info.funds.clone();
    
    // Find the fee coin in the funds
    let fee_coin = funds.iter().find(|coin| coin.denom == "uzig");
    match fee_coin {
        Some(coin) => {
            if coin.amount < pool_creation_fee {
                return Err(ContractError::InvalidPoolCreationFee { 
                    required: pool_creation_fee 
                });
            }
        }
        None => {
            return Err(ContractError::MissingPoolCreationFee {});
        }
    }

    let pair_key = pair_key(&asset_infos, &pair_type);
    TMP_PAIR_INFO.save(deps.storage, &TmpPairInfo { pair_key })?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_PAIR_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pair_config.code_id,
            msg: to_json_binary(&PairInstantiateMsg {
                pair_type: pair_type.clone(),
                asset_infos: asset_infos.clone(),
                token_code_id: config.token_code_id,
                factory_addr: env.contract.address.to_string(),
                init_params,
            })?,
            funds: funds.clone(),
            label: format!("Oroswap pair {}", pair_type),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_attributes(vec![
            attr("action", "create_pair"),
            attr("pair", asset_infos.iter().join("-")),
            attr("pair_type", pair_type.to_string()),
            attr("pool_creation_fee", pool_creation_fee.to_string()),
            attr("total_funds", funds.iter().map(|c| c.amount.to_string()).join(",")),
        ]))
}

/// The entry point to the contract for processing replies from submessages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg {
        Reply {
            id: INSTANTIATE_PAIR_REPLY_ID,
            result:
                SubMsgResult::Ok(SubMsgResponse {
                    data: Some(data), ..
                }),
        } => {
            let tmp = TMP_PAIR_INFO.load(deps.storage)?;
            if PAIRS.has(deps.storage, &tmp.pair_key) {
                return Err(ContractError::PairWasRegistered {});
            }

            let init_response = parse_instantiate_response_data(data.as_slice())
                .map_err(|e| StdError::generic_err(format!("{e}")))?;

            let pair_contract = deps.api.addr_validate(&init_response.contract_address)?;

            PAIRS.save(deps.storage, &tmp.pair_key, &pair_contract)?;

            Ok(Response::new().add_attributes(vec![
                attr("action", "register"),
                attr("pair_contract_addr", pair_contract),
            ]))
        }
        _ => Err(ContractError::FailedToParseReply {}),
    }
}

/// Removes an existing pair from the factory.
///
/// * **asset_infos** is a vector with assets for which we deregister the pair.
///
/// ## Executor
/// Only the owner can execute this.
pub fn deregister(
    deps: DepsMut,
    info: MessageInfo,
    asset_infos: Vec<AssetInfo>,
    pair_type: PairType,
) -> Result<Response, ContractError> {
    check_asset_infos(deps.api, &asset_infos)?;

    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let pair_addr = PAIRS.load(deps.storage, &pair_key(&asset_infos, &pair_type))?;
    PAIRS.remove(deps.storage, &pair_key(&asset_infos, &pair_type));

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(generator) = config.generator_address {
        let pair_info = query_pair_info(&deps.querier, &pair_addr)?;

        // sets the allocation point to zero for the lp_token
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: generator.to_string(),
            msg: to_json_binary(&DeactivatePool {
                lp_token: pair_info.liquidity_token.to_string(),
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "deregister"),
        attr("pair_contract_addr", pair_addr),
        attr("pair_type", pair_type.to_string()),
    ]))
}

pub fn update_tracker_config(
    deps: DepsMut,
    info: MessageInfo,
    tracker_code_id: u64,
    token_factory_addr: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    ensure!(info.sender == config.owner, ContractError::Unauthorized {});
    if let Some(mut tracker_config) = TRACKER_CONFIG.may_load(deps.storage)? {
        tracker_config.code_id = tracker_code_id;
        TRACKER_CONFIG.save(deps.storage, &tracker_config)?;
    } else {
        let tokenfactory_tracker =
            token_factory_addr.ok_or(StdError::generic_err("token_factory_addr is required"))?;
        TRACKER_CONFIG.save(
            deps.storage,
            &TrackerConfig {
                code_id: tracker_code_id,
                token_factory_addr: tokenfactory_tracker,
            },
        )?;
    }

    Ok(Response::new()
        .add_attribute("action", "update_tracker_config")
        .add_attribute("code_id", tracker_code_id.to_string()))
}

/// Pause a specific pair by its asset infos.
///
/// * **asset_infos** is a vector with assets for which we pause the pair.
///
/// ## Executor
/// Only the owner or pause authorities can execute this.
pub fn pause_pair(
    deps: DepsMut,
    info: MessageInfo,
    asset_infos: Vec<AssetInfo>,
    pair_type: PairType,
) -> Result<Response, ContractError> {
    check_asset_infos(deps.api, &asset_infos)?;
    check_pause_authority(deps.storage, &info.sender)?;

    let pair_key = pair_key(&asset_infos, &pair_type);
    
    // Check if pair exists
    let pair_addr = PAIRS.load(deps.storage, &pair_key)?;
    
    // Check if already paused
    if PAUSED_PAIRS.has(deps.storage, &pair_key) {
        return Err(ContractError::PairAlreadyPaused {});
    }

    // Mark pair as paused in factory
    PAUSED_PAIRS.save(deps.storage, &pair_key, &())?;

    // Send pause message to pair contract
    let pause_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pair_addr.to_string(),
        msg: to_json_binary(&oroswap::pair::ExecuteMsg::Pause {})?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(pause_msg)
        .add_attributes(vec![
            attr("action", "pause_pair"),
            attr("pair_key", format!("{:?}", pair_key)),
        ]))
}

/// Unpause a specific pair by its asset infos.
///
/// * **asset_infos** is a vector with assets for which we unpause the pair.
///
/// ## Executor
/// Only the factory admin can execute this.
pub fn unpause_pair(
    deps: DepsMut,
    info: MessageInfo,
    asset_infos: Vec<AssetInfo>,
    pair_type: PairType,
) -> Result<Response, ContractError> {
    check_asset_infos(deps.api, &asset_infos)?;
    check_unpause_authority(deps.storage, &info.sender)?;

    let pair_key = pair_key(&asset_infos, &pair_type);
    
    // Check if pair exists
    let pair_addr = PAIRS.load(deps.storage, &pair_key)?;
    
    // Check if not paused
    if !PAUSED_PAIRS.has(deps.storage, &pair_key) {
        return Err(ContractError::PairNotPaused {});
    }

    // Remove pause status from factory
    PAUSED_PAIRS.remove(deps.storage, &pair_key);

    // Send unpause message to pair contract
    let unpause_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pair_addr.to_string(),
        msg: to_json_binary(&oroswap::pair::ExecuteMsg::Unpause {})?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(unpause_msg)
        .add_attributes(vec![
            attr("action", "unpause_pair"),
            attr("pair_key", format!("{:?}", pair_key)),
        ]))
}

/// Pause pairs in batches (for large numbers of pairs).
///
/// * **batch_size** is the number of pairs to process per batch (default: 50, max: 100).
///
/// ## Executor
/// Only the owner or pause authorities can execute this.
pub fn pause_pairs_batch(
    deps: DepsMut,
    info: MessageInfo,
    batch_size: Option<u32>,
) -> Result<Response, ContractError> {
    check_pause_authority(deps.storage, &info.sender)?;

    let batch_size = batch_size.unwrap_or(50).min(100); // Default 50, max 100
    let mut messages: Vec<CosmosMsg> = vec![];
    let mut paused_count = 0;

    // Collect unpaused pair keys first to avoid borrow checker issues
    let unpaused_pairs: Vec<(Vec<u8>, Addr)> = PAIRS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|result| {
            let (pair_key, pair_addr) = result.ok()?;
            if !PAUSED_PAIRS.has(deps.storage, &pair_key) {
                Some((pair_key, pair_addr))
            } else {
                None
            }
        })
        .take(batch_size as usize)
        .collect();

    let total_pairs = PAIRS.range(deps.storage, None, None, Order::Ascending).count();
    let total_paused = PAUSED_PAIRS.range(deps.storage, None, None, Order::Ascending).count();
    let has_more = total_pairs > total_paused + unpaused_pairs.len();

    // Process the collected pairs
    for (pair_key, pair_addr) in unpaused_pairs {
        // Mark pair as paused in factory
        PAUSED_PAIRS.save(deps.storage, &pair_key, &())?;

        // Add pause message for pair contract
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_addr.to_string(),
            msg: to_json_binary(&oroswap::pair::ExecuteMsg::Pause {})?,
            funds: vec![],
        }));

        paused_count += 1;
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            attr("action", "pause_pairs_batch"),
            attr("paused_count", paused_count.to_string()),
            attr("processed_count", paused_count.to_string()),
            attr("batch_size", batch_size.to_string()),
            attr("has_more", has_more.to_string()),
        ]))
}

/// Unpause pairs in batches (for large numbers of pairs).
///
/// * **batch_size** is the number of pairs to process per batch (default: 50, max: 100).
///
/// ## Executor
/// Only the factory admin can execute this.
pub fn unpause_pairs_batch(
    deps: DepsMut,
    info: MessageInfo,
    batch_size: Option<u32>,
) -> Result<Response, ContractError> {
    check_unpause_authority(deps.storage, &info.sender)?;

    let batch_size = batch_size.unwrap_or(50).min(100); // Default 50, max 100
    let mut messages: Vec<CosmosMsg> = vec![];
    let mut unpaused_count = 0;

    // Collect paused pair keys first to avoid borrow checker issues
    let paused_keys: Vec<Vec<u8>> = PAUSED_PAIRS
        .range(deps.storage, None, None, Order::Ascending)
        .take(batch_size as usize)
        .map(|result| Ok::<Vec<u8>, StdError>(result?.0))
        .collect::<Result<Vec<_>, _>>()?;

    let total_paused = PAUSED_PAIRS
        .range(deps.storage, None, None, Order::Ascending)
        .count();

    let has_more = total_paused > paused_keys.len();

    // Process the collected keys
    for pair_key in paused_keys {
        // Get pair address
        let pair_addr = PAIRS.load(deps.storage, &pair_key)?;

        // Remove pause status from factory
        PAUSED_PAIRS.remove(deps.storage, &pair_key);

        // Add unpause message for pair contract
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_addr.to_string(),
            msg: to_json_binary(&oroswap::pair::ExecuteMsg::Unpause {})?,
            funds: vec![],
        }));

        unpaused_count += 1;
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            attr("action", "unpause_pairs_batch"),
            attr("unpaused_count", unpaused_count.to_string()),
            attr("processed_count", unpaused_count.to_string()),
            attr("batch_size", batch_size.to_string()),
            attr("has_more", has_more.to_string()),
        ]))
}

/// Add addresses with pause authority.
///
/// * **authorities** is a vector of addresses to add as pause authorities.
///
/// ## Executor
/// Only the owner can execute this.
pub fn add_pause_authorities(
    deps: DepsMut,
    info: MessageInfo,
    authorities: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut added_count = 0;
    for authority_str in authorities {
        let authority = deps.api.addr_validate(&authority_str)
            .map_err(|_| ContractError::InvalidPauseAuthority { address: authority_str.clone() })?;
        
        if !PAUSE_AUTHORITIES.has(deps.storage, &authority) {
            PAUSE_AUTHORITIES.save(deps.storage, &authority, &())?;
            added_count += 1;
        }
    }

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "add_pause_authorities"),
            attr("added_count", added_count.to_string()),
        ]))
}

/// Remove addresses with pause authority.
///
/// * **authorities** is a vector of addresses to remove from pause authorities.
///
/// ## Executor
/// Only the owner can execute this.
pub fn remove_pause_authorities(
    deps: DepsMut,
    info: MessageInfo,
    authorities: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut removed_count = 0;
    for authority_str in authorities {
        let authority = deps.api.addr_validate(&authority_str)
            .map_err(|_| ContractError::InvalidPauseAuthority { address: authority_str.clone() })?;
        
        if PAUSE_AUTHORITIES.has(deps.storage, &authority) {
            PAUSE_AUTHORITIES.remove(deps.storage, &authority);
            removed_count += 1;
        }
    }

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "remove_pause_authorities"),
            attr("removed_count", removed_count.to_string()),
        ]))
}

/// Check if the sender has pause authority (owner or pause authority).
fn check_pause_authority(
    storage: &dyn cosmwasm_std::Storage,
    sender: &Addr,
) -> Result<(), ContractError> {
    let config = CONFIG.load(storage)?;
    
    // Owner always has pause authority
    if sender == &config.owner {
        return Ok(());
    }
    
    // Check if sender is a pause authority
    if PAUSE_AUTHORITIES.has(storage, sender) {
        return Ok(());
    }
    
    Err(ContractError::NoPauseAuthority {})
}

/// Check if the sender has unpause authority (only factory admin).
fn check_unpause_authority(
    storage: &dyn cosmwasm_std::Storage,
    sender: &Addr,
) -> Result<(), ContractError> {
    let config = CONFIG.load(storage)?;
    
    // Only owner (factory admin) can unpause
    if sender == &config.owner {
        return Ok(());
    }
    
    Err(ContractError::NoUnpauseAuthority {})
}

/// Exposes all the queries available in the contract.
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns general contract parameters using a custom [`ConfigResponse`] structure.
///
/// * **QueryMsg::Pair { asset_infos }** Returns a [`PairInfo`] object with information about a specific Oroswap pair.
///
/// * **QueryMsg::Pairs { start_after, limit }** Returns an array that contains items of type [`PairInfo`].
/// This returns information about multiple Oroswap pairs
///
/// * **QueryMsg::FeeInfo { pair_type }** Returns the fee structure (total and maker fees) for a specific pair type.
///
/// * **QueryMsg::BlacklistedPairTypes {}** Returns a vector that contains blacklisted pair types (pair types that cannot get ORO emissions).
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Pair { asset_infos, pair_type } => to_json_binary(&query_pair(deps, asset_infos, pair_type)?),
        QueryMsg::Pairs { start_after, limit } => {
            to_json_binary(&query_pairs(deps, start_after, limit)?)
        }
        QueryMsg::FeeInfo { pair_type } => to_json_binary(&query_fee_info(deps, pair_type)?),
        QueryMsg::BlacklistedPairTypes {} => to_json_binary(&query_blacklisted_pair_types(deps)?),
        QueryMsg::TrackerConfig {} => to_json_binary(&query_tracker_config(deps)?),
        QueryMsg::PairsByAssets { asset_infos } => {
            to_json_binary(&query_pairs_by_assets(deps, asset_infos)?)
        }
        QueryMsg::IsPairPaused { asset_infos, pair_type } => to_json_binary(&query_is_pair_paused(deps, asset_infos, pair_type)?),
        QueryMsg::PauseAuthorities {} => to_json_binary(&query_pause_authorities(deps)?),
        QueryMsg::PausedPairsCount {} => to_json_binary(&query_paused_pairs_count(deps)?),
    }
}

/// Returns a vector that contains blacklisted pair types
pub fn query_blacklisted_pair_types(deps: Deps) -> StdResult<Vec<PairType>> {
    PAIR_CONFIGS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|result| match result {
            Ok(v) => {
                if v.1.is_disabled || v.1.is_generator_disabled {
                    Some(Ok(v.1.pair_type))
                } else {
                    None
                }
            }
            Err(e) => Some(Err(e)),
        })
        .collect()
}

/// Returns general contract parameters using a custom [`ConfigResponse`] structure.
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: config.owner,
        token_code_id: config.token_code_id,
        pair_configs: PAIR_CONFIGS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| Ok(item?.1))
            .collect::<StdResult<Vec<_>>>()?,
        fee_address: config.fee_address,
        generator_address: config.generator_address,
        whitelist_code_id: config.whitelist_code_id,
        coin_registry_address: config.coin_registry_address,
    };

    Ok(resp)
}

/// Returns a pair's data using the assets in `asset_infos` as input (those being the assets that are traded in the pair).
/// * **asset_infos** is a vector with assets traded in the pair.
pub fn query_pair(deps: Deps, asset_infos: Vec<AssetInfo>, pair_type: PairType) -> StdResult<PairInfo> {
    let pair_addr = PAIRS.load(deps.storage, &pair_key(&asset_infos, &pair_type))?;
    query_pair_info(&deps.querier, pair_addr)
}

/// Returns a vector with pair data that contains items of type [`PairInfo`]. Querying starts at `start_after` and returns `limit` pairs.
/// * **start_after** is a field which accepts a vector with items of type [`AssetInfo`].
/// This is the pair from which we start a query.
///
/// * **limit** sets the number of pairs to be retrieved.
pub fn query_pairs(
    deps: Deps,
    start_after: Option<StartAfter>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let pairs = read_pairs(deps, start_after, limit)?
        .iter()
        .map(|pair_addr| query_pair_info(&deps.querier, pair_addr))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(PairsResponse { pairs })
}

/// Returns the fee setup for a specific pair type using a [`FeeInfoResponse`] struct.
/// * **pair_type** is a struct that represents the fee information (total and maker fees) for a specific pair type.
pub fn query_fee_info(deps: Deps, pair_type: PairType) -> StdResult<FeeInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let pair_config = PAIR_CONFIGS.load(deps.storage, pair_type.to_string())?;

    Ok(FeeInfoResponse {
        fee_address: config.fee_address,
        total_fee_bps: pair_config.total_fee_bps,
        maker_fee_bps: pair_config.maker_fee_bps,
        pool_creation_fee: pair_config.pool_creation_fee,
    })
}

pub fn query_tracker_config(deps: Deps) -> StdResult<TrackerConfig> {
    let tracker_config = TRACKER_CONFIG.load(deps.storage).map_err(|_| {
        StdError::generic_err("Tracker config is not set in the factory. It can't be provided")
    })?;

    Ok(TrackerConfig {
        code_id: tracker_config.code_id,
        token_factory_addr: tracker_config.token_factory_addr,
    })
}

/// Returns whether a specific pair is paused.
/// * **asset_infos** is a vector with assets for which we check if the pair is paused.
pub fn query_is_pair_paused(deps: Deps, asset_infos: Vec<AssetInfo>, pair_type: PairType) -> StdResult<bool> {
    check_asset_infos(deps.api, &asset_infos).map_err(|e| StdError::generic_err(e.to_string()))?;
    let pair_key = pair_key(&asset_infos, &pair_type);
    Ok(PAUSED_PAIRS.has(deps.storage, &pair_key))
}

/// Returns all pause authorities.
pub fn query_pause_authorities(deps: Deps) -> StdResult<Vec<String>> {
    let authorities: Vec<String> = PAUSE_AUTHORITIES
        .range(deps.storage, None, None, Order::Ascending)
        .map(|result| Ok(result?.0.to_string()))
        .collect::<StdResult<Vec<_>>>()?;
    Ok(authorities)
}

/// Returns the total number of paused pairs.
pub fn query_paused_pairs_count(deps: Deps) -> StdResult<u32> {
    let count = PAUSED_PAIRS
        .range(deps.storage, None, None, Order::Ascending)
        .fold(Ok(0u32), |acc: StdResult<u32>, _res| Ok(acc? + 1))?;
    Ok(count)
}

/// Manages the contract migration.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut response = Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION);

    // Get all pairs from storage
    let pairs: Vec<(Vec<u8>, Addr)> = PAIRS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    // For each pair, check if it needs migration
    for (raw_key, pair_addr) in pairs {
        // Get actual pair info from contract
        let pair_info = query_pair_info(&deps.querier, &pair_addr)?;
        
        // Compute what the key should be
        let expected_key = pair_key(&pair_info.asset_infos, &pair_info.pair_type);

        // If key is already correct, skip
        if raw_key == expected_key {
            continue;
        }

        // Otherwise migrate to new key
        PAIRS.save(deps.storage, &expected_key, &pair_addr)?;
        PAIRS.remove(deps.storage, &raw_key);

        response = response.add_attribute("migrated_pair", pair_addr.to_string());
    }

    Ok(response)
}

/// Returns a vector of pairs that contain items of type [`PairInfo`].
/// This returns information about multiple Oroswap pairs for a given set of assets.
pub fn query_pairs_by_assets(deps: Deps, asset_infos: Vec<AssetInfo>) -> StdResult<PairsResponse> {
    // Get all pair types from config
    let pair_types = PAIR_CONFIGS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| Ok(item?.1.pair_type))
        .collect::<StdResult<Vec<_>>>()?;

    // Try to find pairs for each pair type
    let mut pairs = Vec::new();
    for pair_type in pair_types {
        if let Ok(pair_addr) = PAIRS.load(deps.storage, &pair_key(&asset_infos, &pair_type)) {
            if let Ok(pair_info) = query_pair_info(&deps.querier, pair_addr) {
                pairs.push(pair_info);
            }
        }
    }

    Ok(PairsResponse { pairs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use oroswap::asset::{native_asset_info, token_asset_info, PairInfo};
    use crate::mock_querier::mock_dependencies as mock_deps;

    #[test]
    fn test_migration_with_legacy_pairs() {
        let mut deps = mock_deps(&[]);
        
        // Setup initial contract version
        set_contract_version(deps.as_mut().storage, CONTRACT_NAME, "1.0.0").unwrap();

        // Create test pairs with old keys (without pair type)
        let asset_infos1 = vec![
            native_asset_info("uluna".to_string()),
            native_asset_info("uusd".to_string()),
        ];
        let asset_infos2 = vec![
            native_asset_info("uluna".to_string()),
            token_asset_info(Addr::unchecked("token_addr")),
        ];
        let asset_infos3 = vec![
            native_asset_info("uusd".to_string()),
            token_asset_info(Addr::unchecked("token_addr")),
        ];

        // Save pairs with old keys
        let pair1_addr = Addr::unchecked("pair1");
        let pair2_addr = Addr::unchecked("pair2");

        // Create old keys (without pair type)
        let mut sorted_asset_infos1 = asset_infos1.clone();
        sorted_asset_infos1.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        let old_key1 = [sorted_asset_infos1[0].as_bytes(), sorted_asset_infos1[1].as_bytes()].concat();

        let mut sorted_asset_infos2 = asset_infos2.clone();
        sorted_asset_infos2.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        let old_key2 = [sorted_asset_infos2[0].as_bytes(), sorted_asset_infos2[1].as_bytes()].concat();

        // Save pairs with old keys
        PAIRS.save(deps.as_mut().storage, &old_key1, &pair1_addr).unwrap();
        PAIRS.save(deps.as_mut().storage, &old_key2, &pair2_addr).unwrap();

        // Also save pairs with new key formats to test skipping
        // 1. Xyk pair in new format
        let new_key = pair_key(&asset_infos3, &PairType::Xyk {});
        let pair3_addr = Addr::unchecked("pair3");
        PAIRS.save(deps.as_mut().storage, &new_key, &pair3_addr).unwrap();

        // 2. Custom pair in new format
        let custom_key = pair_key(&asset_infos1, &PairType::Custom("concentrated".to_string()));
        let pair4_addr = Addr::unchecked("pair4");
        PAIRS.save(deps.as_mut().storage, &custom_key, &pair4_addr).unwrap();

        // 3. Custom xyk_30 pair in new format
        let xyk_30_key = pair_key(&asset_infos2, &PairType::Custom("xyk_30".to_string()));
        let pair5_addr = Addr::unchecked("pair5");
        PAIRS.save(deps.as_mut().storage, &xyk_30_key, &pair5_addr).unwrap();

        // Mock pair contract responses with explicit pair types
        deps.querier.with_oroswap_pairs(&[
            (
                &pair1_addr.to_string(),
                &PairInfo {
                    asset_infos: asset_infos1.clone(),
                    contract_addr: pair1_addr.clone(),
                    liquidity_token: "liquidity1".to_string(),
                    pair_type: PairType::Xyk {}, // Explicit pair type
                },
            ),
            (
                &pair2_addr.to_string(),
                &PairInfo {
                    asset_infos: asset_infos2.clone(),
                    contract_addr: pair2_addr.clone(),
                    liquidity_token: "liquidity2".to_string(),
                    pair_type: PairType::Xyk {}, // Explicit pair type
                },
            ),
            (
                &pair3_addr.to_string(),
                &PairInfo {
                    asset_infos: asset_infos3.clone(),
                    contract_addr: pair3_addr.clone(),
                    liquidity_token: "liquidity3".to_string(),
                    pair_type: PairType::Xyk {},
                },
            ),
            (
                &pair4_addr.to_string(),
                &PairInfo {
                    asset_infos: asset_infos1.clone(),
                    contract_addr: pair4_addr.clone(),
                    liquidity_token: "liquidity4".to_string(),
                    pair_type: PairType::Custom("concentrated".to_string()),
                },
            ),
            (
                &pair5_addr.to_string(),
                &PairInfo {
                    asset_infos: asset_infos2.clone(),
                    contract_addr: pair5_addr.clone(),
                    liquidity_token: "liquidity5".to_string(),
                    pair_type: PairType::Custom("xyk_30".to_string()),
                },
            ),
        ]);

        // Run migration
        let res = migrate(deps.as_mut(), mock_env(), MigrateMsg { tracker_config: None }).unwrap();

        // Verify migration attributes
        assert!(res.attributes.iter().any(|attr| 
            attr.key == "migrated_pair" && attr.value == "pair1"
        ));
        assert!(res.attributes.iter().any(|attr| 
            attr.key == "migrated_pair" && attr.value == "pair2"
        ));
        // Verify pairs with new format are not migrated
        assert!(!res.attributes.iter().any(|attr| 
            attr.key == "migrated_pair" && attr.value == "pair3"
        ));
        assert!(!res.attributes.iter().any(|attr| 
            attr.key == "migrated_pair" && attr.value == "pair4"
        ));
        assert!(!res.attributes.iter().any(|attr| 
            attr.key == "migrated_pair" && attr.value == "pair5"
        ));

        // Verify old keys are removed
        assert!(!PAIRS.has(deps.as_ref().storage, &old_key1));
        assert!(!PAIRS.has(deps.as_ref().storage, &old_key2));

        // Verify new keys exist with correct pair type
        let new_key1 = pair_key(&asset_infos1, &PairType::Xyk {});
        let new_key2 = pair_key(&asset_infos2, &PairType::Xyk {});

        assert_eq!(PAIRS.load(deps.as_ref().storage, &new_key1).unwrap(), pair1_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &new_key2).unwrap(), pair2_addr);
        
        // Verify pairs with new format are unchanged
        assert_eq!(PAIRS.load(deps.as_ref().storage, &new_key).unwrap(), pair3_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &custom_key).unwrap(), pair4_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &xyk_30_key).unwrap(), pair5_addr);

        // Test double migration
        let res2 = migrate(deps.as_mut(), mock_env(), MigrateMsg { tracker_config: None }).unwrap();
        
        // Verify no pairs were migrated in second run
        assert!(!res2.attributes.iter().any(|attr| attr.key == "migrated_pair"));
        
        // Verify all keys remain unchanged
        assert_eq!(PAIRS.load(deps.as_ref().storage, &new_key1).unwrap(), pair1_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &new_key2).unwrap(), pair2_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &new_key).unwrap(), pair3_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &custom_key).unwrap(), pair4_addr);
        assert_eq!(PAIRS.load(deps.as_ref().storage, &xyk_30_key).unwrap(), pair5_addr);
    }
}
