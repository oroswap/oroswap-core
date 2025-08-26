use cosmwasm_std::{
    attr, entry_point, to_json_binary, Binary, Deps, DepsMut, Empty, Env,
    MessageInfo, Reply, Response, StdResult, SubMsg, WasmMsg, Uint128,
};

use oroswap_core::asset::AssetInfo;
use oroswap_core::factory::{ExecuteMsg as FactoryExecuteMsg, PairType, QueryMsg as FactoryQueryMsg};
use oroswap_core::pair::ExecuteMsg as PairExecuteMsg;
use cw20::Cw20ExecuteMsg;
use std::collections::BTreeMap;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, ProvideLiquidityParams};
use crate::state::{
    Config, PendingLiquidity, CONFIG, CREATE_PAIR_REPLY_ID, PENDING_LIQUIDITY, PROVIDE_LIQUIDITY_REPLY_ID,
};

/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        owner: info.sender.clone(),
        factory_addr: deps.api.addr_validate(&msg.factory_addr)?,
        pair_creation_fee: msg.pair_creation_fee,
        fee_denom: msg.fee_denom,
    };
    CONFIG.save(deps.storage, &config)?;

    cw2::set_contract_version(deps.storage, crate::state::CONTRACT_NAME, crate::state::CONTRACT_VERSION)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "instantiate"),
        attr("factory_addr", msg.factory_addr.to_string()),
    ]))
}

/// Exposes all the execute functions available in the contract.
#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreatePairAndProvideLiquidity {
            pair_type,
            asset_infos,
            init_params,
            liquidity,
        } => execute_create_pair_and_provide_liquidity(
            deps,
            _env,
            info,
            pair_type,
            asset_infos,
            init_params,
            liquidity,
        ),
        ExecuteMsg::UpdateConfig {
            factory_addr,
            pair_creation_fee,
            fee_denom,
        } => execute_update_config(deps, _env, info, factory_addr, pair_creation_fee, fee_denom),
        ExecuteMsg::EmergencyRecovery {} => execute_emergency_recovery(deps, _env, info),
    }
}

/// Exposes all the query functions available in the contract.
#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),

    }
}

/// The entry point to the contract for processing replies from submessages.
#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        CREATE_PAIR_REPLY_ID => {
            // Factory doesn't return pair address in reply data, so we need to query it
            let pending = PENDING_LIQUIDITY.load(deps.storage)?;
            let factory_addr = CONFIG.load(deps.storage)?.factory_addr;
            
            // Query the factory to get the pair address
            let query_msg = FactoryQueryMsg::Pair {
                asset_infos: pending.asset_infos.clone(),
                pair_type: pending.pair_type.clone(),
            };
            
            let pair_info: oroswap_core::asset::PairInfo = deps.querier.query_wasm_smart(
                factory_addr,
                &query_msg,
            ).map_err(|_| ContractError::FailedToQueryFactory {})?;
            
            // Use the pair address directly (it's already validated by the factory)
            let pair_addr = pair_info.contract_addr;
            
            // Create submessages for CW-20 token transfers and approvals
            let mut submessages = vec![];
            
            // First, transfer CW-20 tokens from user to pool initializer
            for (cw20_contract, amount) in &pending.cw20_messages {
                let transfer_msg = Cw20ExecuteMsg::TransferFrom {
                    owner: pending.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: *amount,
                };
                
                submessages.push(SubMsg::new(WasmMsg::Execute {
                    contract_addr: cw20_contract.to_string(),
                    msg: to_json_binary(&transfer_msg)?,
                    funds: vec![],
                }));
            }
            
            // Then, approve the pair contract to spend the CW-20 tokens
            for (cw20_contract, amount) in &pending.cw20_messages {
                let approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
                    spender: pair_addr.to_string(),
                    amount: *amount,
                    expires: None,
                };
                
                submessages.push(SubMsg::new(WasmMsg::Execute {
                    contract_addr: cw20_contract.to_string(),
                    msg: to_json_binary(&approve_msg)?,
                    funds: vec![],
                }));
            }
            
            // Prepare provide liquidity msg
            let receiver = pending
                .liquidity
                .receiver
                .clone()
                .or_else(|| Some(pending.sender.to_string()));
            let provide_liquidity_msg = PairExecuteMsg::ProvideLiquidity {
                assets: pending.liquidity.assets,
                slippage_tolerance: pending.liquidity.slippage_tolerance,
                auto_stake: None,
                receiver,
                min_lp_to_receive: pending.liquidity.min_lp_to_receive,
            };
            
            // Add the provide liquidity message (with native token funds if any)
            submessages.push(SubMsg::reply_on_success(
                WasmMsg::Execute {
                    contract_addr: pair_addr.to_string(),
                    msg: to_json_binary(&provide_liquidity_msg)?,
                    funds: pending.funds,
                },
                PROVIDE_LIQUIDITY_REPLY_ID,
            ));
            
            Ok(Response::new()
                .add_submessages(submessages)
                .add_attributes(vec![
                    attr("action", "pair_created"),
                    attr("pair_addr", pair_addr),
                ]))
        }
        PROVIDE_LIQUIDITY_REPLY_ID => {
            // Clean up state
            PENDING_LIQUIDITY.remove(deps.storage);
            Ok(Response::new()
                .add_attributes(vec![
                    attr("action", "liquidity_provided"),
                ]))
        }
        _ => Err(ContractError::InvalidReplyId {}),
    }
}

/// The entry point to the contract for migrations.
#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: Empty) -> Result<Response, ContractError> {
    let contract_version = cw2::get_contract_version(deps.storage)?;

    cw2::set_contract_version(deps.storage, crate::state::CONTRACT_NAME, crate::state::CONTRACT_VERSION)?;
    
    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", crate::state::CONTRACT_NAME)
        .add_attribute("new_contract_version", crate::state::CONTRACT_VERSION))
}

/// Helper function to add amounts to a BTreeMap
fn add_amount(m: &mut BTreeMap<String, Uint128>, k: &str, v: Uint128) {
    m.entry(k.to_string()).and_modify(|x| *x += v).or_insert(v);
}

/// Create a pair and provide liquidity in a single atomic transaction
pub fn execute_create_pair_and_provide_liquidity(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pair_type: PairType,
    asset_infos: Vec<AssetInfo>,
    init_params: Option<Binary>,
    liquidity: ProvideLiquidityParams,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Defensive guard: ensure no pending operation exists
    if PENDING_LIQUIDITY.may_load(deps.storage)?.is_some() {
        return Err(ContractError::OperationInProgress {});
    }

    // Validate asset infos
    if asset_infos.len() != 2 {
        return Err(ContractError::InvalidAssetInfo {});
    }

    // Validate initial liquidity matches asset infos
    if liquidity.assets.len() != 2 {
        return Err(ContractError::InvalidInitialLiquidity {});
    }

    // Asset consistency checks
    // 1. Assets must match the pair assets 1:1 and be distinct
    for (i, ai) in asset_infos.iter().enumerate() {
        if liquidity.assets[i].info != *ai {
            return Err(ContractError::AssetMismatch {});
        }
    }
    
    // 2. Assets must be distinct
    if asset_infos[0] == asset_infos[1] {
        return Err(ContractError::InvalidAssetInfo {});
    }
    
    // 3. Disallow zero amounts
    for a in &liquidity.assets {
        if a.amount.is_zero() {
            return Err(ContractError::InvalidInitialLiquidity {});
        }
    }

    // 4. Validate native coverage
    let mut need = BTreeMap::<String, Uint128>::new();
    for a in &liquidity.assets {
        if let AssetInfo::NativeToken { denom } = &a.info {
            add_amount(&mut need, denom, a.amount);
        }
    }
    let mut sent = BTreeMap::<String, Uint128>::new();
    for c in &info.funds {
        add_amount(&mut sent, &c.denom, c.amount);
    }
    for (denom, req) in need {
        let got = sent.get(&denom).cloned().unwrap_or_default();
        if got < req {
            return Err(ContractError::InsufficientFundsForDenom { denom });
        }
    }

    // 5. Enforce minimum pool creation fee
    let fee_denom = config.fee_denom.clone();
    let fee_sent = info.funds.iter()
        .find(|c| c.denom == fee_denom)
        .map(|c| c.amount)
        .unwrap_or_default();
    if fee_sent < config.pair_creation_fee {
        return Err(ContractError::InsufficientFundsForDenom { denom: fee_denom });
    }

    // Extract pool creation fee and keep the rest for liquidity
    let mut factory_funds = vec![];
    let mut liquidity_funds = vec![];
    let mut cw20_messages = vec![];
    
    for coin in &info.funds {
        if coin.denom == fee_denom {
            // Since we already validated fee_sent >= config.pair_creation_fee,
            // we can safely extract the fee and treat remainder as liquidity
            factory_funds.push(cosmwasm_std::Coin {
                denom: fee_denom.clone(),
                amount: config.pair_creation_fee,
            });
            // Keep the rest for liquidity
            let remaining = coin.amount - config.pair_creation_fee;
            if !remaining.is_zero() {
                liquidity_funds.push(cosmwasm_std::Coin {
                    denom: fee_denom.clone(),
                    amount: remaining,
                });
            }
        } else {
            // Non-fee tokens go to liquidity
            liquidity_funds.push(coin.clone());
        }
    }
    
    // Handle CW-20 tokens from liquidity.assets
    for asset in &liquidity.assets {
        if let AssetInfo::Token { contract_addr } = &asset.info {
            // For CW-20 tokens, we need to send them via Cw20ExecuteMsg::Send
            // We'll store this information and handle it in the reply handler
            cw20_messages.push((
                contract_addr.clone(),
                asset.amount,
            ));
        }
    }
    
    // Save pending operation
    let pending = PendingLiquidity {
        sender: info.sender.clone(),
        pair_type: pair_type.clone(),
        asset_infos: asset_infos.clone(),
        init_params: init_params.clone(),
        liquidity,
        funds: liquidity_funds, // Store the native token funds
        cw20_messages, // Store CW-20 messages to send
    };
    PENDING_LIQUIDITY.save(deps.storage, &pending)?;
    
    // Send submsg to factory to create pair
    let msg = FactoryExecuteMsg::CreatePair {
        pair_type,
        asset_infos,
        init_params,
    };
    let submsg = SubMsg::reply_on_success(
        WasmMsg::Execute {
            contract_addr: config.factory_addr.to_string(),
            msg: to_json_binary(&msg)?,
            funds: factory_funds, // Send pool creation fee to factory
        },
        CREATE_PAIR_REPLY_ID,
    );
    Ok(Response::new()
        .add_submessage(submsg)
        .add_attributes(vec![
            attr("action", "create_pair_and_provide_liquidity"),
            attr("sender", info.sender),
        ]))
}

/// Update the contract configuration (admin only)
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    factory_addr: Option<String>,
    pair_creation_fee: Option<Uint128>,
    fee_denom: Option<String>,
) -> Result<Response, ContractError> {
    // Only the contract admin can update config
    let config = CONFIG.load(deps.storage)?;
    
    // Check if sender is the contract owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    let mut new_config = config;
    
    if let Some(addr) = factory_addr {
        new_config.factory_addr = deps.api.addr_validate(&addr)?;
    }
    
    if let Some(fee) = pair_creation_fee {
        new_config.pair_creation_fee = fee;
    }
    
    if let Some(denom) = fee_denom {
        new_config.fee_denom = denom;
    }
    
    CONFIG.save(deps.storage, &new_config)?;
    
    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "update_config"),
            attr("factory_addr", new_config.factory_addr.to_string()),
            attr("pair_creation_fee", new_config.pair_creation_fee.to_string()),
            attr("fee_denom", new_config.fee_denom.clone()),
        ]))
}

/// Query the contract configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        factory_addr: config.factory_addr.to_string(),
        pair_creation_fee: config.pair_creation_fee,
        fee_denom: config.fee_denom,
    })
}

/// Emergency recovery function to clean up stuck operations (admin only)
pub fn execute_emergency_recovery(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    // Clear any stuck pending liquidity operation
    if PENDING_LIQUIDITY.may_load(deps.storage)?.is_some() {
        PENDING_LIQUIDITY.remove(deps.storage);
    }
    
    Ok(Response::new().add_attributes(vec![
        attr("action", "emergency_recovery"),
        attr("admin", info.sender.to_string()),
    ]))
}






