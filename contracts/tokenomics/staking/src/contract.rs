#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, Uint128,
    WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::{must_pay, parse_reply_instantiate_data, MsgInstantiateContractResponse};

use oroswap::staking::{
    Config, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse, StakingResponse, UnbondingEntry,
};
use oroswap::token_factory::{
    tf_burn_msg, tf_create_denom_msg, tf_mint_msg, tf_set_denom_metadata_msg,
    MsgCreateDenomResponse,
};

use crate::error::ContractError;
use crate::state::{CONFIG, TOTAL_LOCKED, UNBONDING};

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
/// Contract version that is used for migration.
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// xORO information
const TOKEN_SYMBOL: &str = "xoro";

/// A `reply` call code ID used for sub-messages.
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

/// Minimum initial xoro share
pub(crate) const MINIMUM_STAKE_AMOUNT: Uint128 = Uint128::new(1_000);

/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate that deposit_token_denom exists on chain
    let supply = deps.querier.query_supply(&msg.deposit_token_denom)?.amount;
    ensure!(
        !supply.is_zero(),
        StdError::generic_err(
            "deposit_token_denom has 0 supply which is likely sign of misconfiguration"
        )
    );

    // Validate addresses
    deps.api.addr_validate(&msg.token_factory_addr)?;

    let owner = deps.api.addr_validate(
        &info.sender.to_string(), // owner defaults to instantiator; can be updated via UpdateConfig
    )?;

    // Set bootstrap amount (default to MINIMUM_STAKE_AMOUNT if not provided)
    let bootstrap_amount = msg.bootstrap_amount.unwrap_or(MINIMUM_STAKE_AMOUNT);

    if let Some(provided_amount) = msg.bootstrap_amount {
        if provided_amount < MINIMUM_STAKE_AMOUNT {
            return Err(StdError::generic_err(format!(
                "Bootstrap amount must be at least {}",
                MINIMUM_STAKE_AMOUNT
            )));
        }
    }

    // Check if bootstrap funds are provided
    let provided_funds = info
        .funds
        .iter()
        .find(|c| c.denom == msg.deposit_token_denom)
        .map(|c| c.amount)
        .unwrap_or_default();

    if provided_funds != bootstrap_amount {
        return Err(StdError::generic_err(format!(
            "Expected bootstrap amount: {}, provided: {}",
            bootstrap_amount, provided_funds
        )));
    }
    let deposit_token_denom = msg.deposit_token_denom.clone();

    CONFIG.save(
        deps.storage,
        &Config {
            oro_denom: deposit_token_denom.clone(),
            xoro_denom: "".to_string(), // placeholder, set in reply
            bootstrap_amount,
            owner: owner.to_string(),
            unbonding_period: msg.unbonding_period,
            xoro_decimals: msg.xoro_decimals.unwrap_or(6),
            xoro_uri: msg.xoro_uri,
            paused: false,
        },
    )?;

    // Initialize total locked to zero
    TOTAL_LOCKED.save(deps.storage, &0u128)?;

    // Store tracker config for queries if needed
    // (We'll just keep the token_factory_addr for reference in tracker_config query if we keep it, but let's skip state entry for now)

    let create_denom_msg = SubMsg::reply_on_success(
        tf_create_denom_msg(env.contract.address.to_string(), TOKEN_SYMBOL),
        ReplyIds::InstantiateDenom as u64,
    );

    Ok(Response::new()
        .add_submessage(create_denom_msg)
        .add_attributes(vec![
            attr("action", "instantiate"),
            attr("contract", CONTRACT_NAME),
            attr("deposit_token_denom", deposit_token_denom),
            attr("unbonding_period", msg.unbonding_period.to_string()),
        ]))
}

/// Exposes execute functions available in the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Enter { receiver } => {
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            execute_enter(deps, env, info).map(|(resp, minted_coins)| {
                resp.add_message(BankMsg::Send {
                    to_address: recipient.clone(),
                    amount: vec![minted_coins],
                })
                .add_attributes([("action", "enter"), ("recipient", recipient.as_str())])
            })
        }
        ExecuteMsg::EnterWithHook {
            contract_address,
            msg,
        } => execute_enter(deps, env, info).map(|(resp, minted_coins)| {
            resp.add_message(WasmMsg::Execute {
                contract_addr: contract_address.clone(),
                msg,
                funds: vec![minted_coins],
            })
            .add_attributes([
                ("action", "enter_with_hook"),
                ("next_contract", &contract_address),
            ])
        }),
        ExecuteMsg::Leave { receiver } => {
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            execute_leave(deps, env, info, recipient)
        }
        ExecuteMsg::Claim { receiver } => {
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            execute_claim(deps, env, info, recipient)
        }
        ExecuteMsg::UpdateConfig {
            owner,
            unbonding_period,
        } => execute_update_config(deps, info, owner, unbonding_period),
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Unpause {} => execute_unpause(deps, info),
    }
}

/// The entry point to the contract for processing replies from submessages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match ReplyIds::try_from(msg.id)? {
        ReplyIds::InstantiateDenom => {
            if let cosmwasm_std::SubMsgResult::Ok(cosmwasm_std::SubMsgResponse {
                data: Some(b), ..
            }) = msg.result
            {
                #[cfg(not(any(feature = "zigchain")))]
                let MsgCreateDenomResponse { new_token_denom } = b.try_into()?;
                #[cfg(feature = "zigchain")]
                let MsgCreateDenomResponse { denom: new_token_denom, .. } = b.try_into()?;

                CONFIG.update::<_, StdError>(deps.storage, |mut config| {
                    config.xoro_denom = new_token_denom.clone();
                    Ok(config)
                })?;

                let config = CONFIG.load(deps.storage)?;
                let set_metadata_msg = tf_set_denom_metadata_msg(
                    env.contract.address.clone(),
                    new_token_denom.clone(),
                    TOKEN_SYMBOL.to_string(),
                    config.xoro_decimals,
                    config.xoro_uri,
                );

                // Bootstrap the pool by minting xORO tokens equal to bootstrap amount
                // Mint to contract itself first, then the ratio is initialized
                let bootstrap_coins = coin(config.bootstrap_amount.u128(), &new_token_denom);
                let bootstrap_mint_msgs = tf_mint_msg(
                    env.contract.address.clone(),
                    bootstrap_coins,
                    env.contract.address.clone(),
                );

                Ok(Response::new()
                    .add_message(set_metadata_msg)
                    .add_messages(bootstrap_mint_msgs)
                    .add_attributes([
                        attr("xoro_denom", new_token_denom),
                        attr("bootstrap_amount", config.bootstrap_amount.to_string()),
                        attr("pool_bootstrapped", "true".to_string()),
                    ]))
            } else {
                Err(ContractError::FailedToParseReply {})
            }
        }
    }
}

/// Returns the effective ORO balance for ratio calculations:
/// `effective = total_balance - total_locked`.
/// This prevents unbonding ORO from diluting rewards for active stakers.
fn effective_deposit(total_balance: Uint128, total_locked: u128) -> Result<Uint128, ContractError> {
    total_balance
        .checked_sub(Uint128::new(total_locked))
        .map_err(|_| {
            ContractError::Std(StdError::generic_err(
                "total_locked exceeds oro_balance; state is inconsistent",
            ))
        })
}

/// Enter stakes TokenFactory ORO for xORO.
fn execute_enter(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<(Response, Coin), ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    // Ensure the correct denom is sent
    let amount = must_pay(&info, &config.oro_denom)?;

    let total_locked = TOTAL_LOCKED.load(deps.storage)?;

    // Compute effective deposit (excluding user's incoming deposit, which is already in balance)
    let raw_balance = deps
        .querier
        .query_balance(&env.contract.address, &config.oro_denom)?
        .amount;

    // Incoming amount has already landed in contract; subtract it to get pre-deposit balance
    let pre_deposit_balance = raw_balance
        .checked_sub(amount)
        .map_err(|e| ContractError::Std(e.into()))?;
    let effective = effective_deposit(pre_deposit_balance, total_locked)?;

    let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;

    let mint_amount = if total_shares.is_zero() || effective.is_zero() {
        amount
    } else {
        amount.multiply_ratio(total_shares, effective)
    };

    if mint_amount.is_zero() {
        return Err(ContractError::StakeAmountTooSmall {});
    }

    let minted_coins = coin(mint_amount.u128(), config.xoro_denom.clone());

    let messages = tf_mint_msg(
        env.contract.address.clone(),
        minted_coins.clone(),
        env.contract.address.clone(),
    );

    let messages: Vec<CosmosMsg> = messages;

    let staking_response = to_json_binary(&StakingResponse {
        oro_amount: amount,
        xoro_amount: mint_amount,
    })?;

    Ok((
        Response::new()
            .add_messages(messages)
            .set_data(staking_response)
            .add_attributes([attr("oro_amount", amount), attr("xoro_amount", mint_amount)]),
        minted_coins,
    ))
}

/// Leave begins the unbonding process: burns xORO immediately, locks ORO for the unbonding period.
fn execute_leave(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    let xoro_amount = must_pay(&info, &config.xoro_denom)?;

    let total_locked = TOTAL_LOCKED.load(deps.storage)?;

    let total_balance = deps
        .querier
        .query_balance(&env.contract.address, &config.oro_denom)?
        .amount;

    let effective = effective_deposit(total_balance, total_locked)?;
    let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;

    // Calculate the ORO return amount from the current effective pool.
    let return_amount = xoro_amount.multiply_ratio(effective, total_shares);

    if return_amount.is_zero() {
        return Err(ContractError::StakeAmountTooSmall {});
    }

    // Burn the xORO immediately
    let burn_msg = tf_burn_msg(
        env.contract.address.clone(),
        coin(xoro_amount.u128(), &config.xoro_denom),
    );

    // Add unbonding entry for the user
    let unlock_time = env.block.time.seconds() + config.unbonding_period;
    let entry = UnbondingEntry {
        oro_amount: return_amount,
        unlock_time,
    };

    let sender = info.sender.clone();
    UNBONDING.update(deps.storage, &sender, |maybe_entries| -> StdResult<_> {
        let mut entries = maybe_entries.unwrap_or_default();
        entries.push(entry);
        Ok(entries)
    })?;

    // Increase TOTAL_LOCKED
    TOTAL_LOCKED.update(deps.storage, |locked| -> StdResult<u128> {
        Ok(locked + return_amount.u128())
    })?;

    let staking_response = to_json_binary(&StakingResponse {
        oro_amount: return_amount,
        xoro_amount: xoro_amount,
    })?;

    Ok(Response::new()
        .add_message(burn_msg)
        .set_data(staking_response)
        .add_attributes([
            attr("action", "leave"),
            attr("sender", &sender),
            attr("recipient", &recipient),
            attr("xoro_amount", xoro_amount),
            attr("oro_locked", return_amount),
            attr("unlock_time", unlock_time.to_string()),
        ]))
}

/// Claim releases all matured unbonding entries to the recipient.
fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;
    let now = env.block.time.seconds();

    let sender = info.sender.clone();
    let entries = UNBONDING
        .may_load(deps.storage, &sender)?
        .unwrap_or_default();

    let (matured, pending): (Vec<UnbondingEntry>, Vec<UnbondingEntry>) =
        entries.into_iter().partition(|e| e.unlock_time <= now);

    if matured.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(
            "No matured unbonding entries to claim",
        )));
    }

    let total_claim: Uint128 = matured.iter().map(|e| e.oro_amount).sum();

    // Update state: keep only pending entries
    if pending.is_empty() {
        UNBONDING.remove(deps.storage, &sender);
    } else {
        UNBONDING.save(deps.storage, &sender, &pending)?;
    }

    // Reduce TOTAL_LOCKED
    TOTAL_LOCKED.update(deps.storage, |locked| -> StdResult<u128> {
        Ok(locked.saturating_sub(total_claim.u128()))
    })?;

    // Send ORO to recipient
    let send_msg: CosmosMsg = BankMsg::Send {
        to_address: recipient.clone(),
        amount: vec![coin(total_claim.u128(), &config.oro_denom)],
    }
    .into();

    Ok(Response::new()
        .add_message(send_msg)
        .add_attributes([
            attr("action", "claim"),
            attr("sender", &sender),
            attr("recipient", &recipient),
            attr("oro_claimed", total_claim),
            attr("entries_claimed", matured.len().to_string()),
        ]))
}


/// Updates the contract configuration. Owner only.
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    unbonding_period: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(new_owner) = owner {
        deps.api.addr_validate(&new_owner)?;
        config.owner = new_owner;
    }
    if let Some(period) = unbonding_period {
        // Optional: enforce a maximum cap, e.g. 30 days
        const MAX_UNBONDING_PERIOD: u64 = 30 * 24 * 60 * 60; // 30 days
        if period > MAX_UNBONDING_PERIOD {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "unbonding_period cannot exceed {} seconds (30 days)",
                MAX_UNBONDING_PERIOD
            ))));
        }
        config.unbonding_period = period;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Pauses the contract. Owner only.
fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    config.paused = true;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "pause"))
}

/// Unpauses the contract. Owner only.
fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    config.paused = false;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "unpause"))
}

/// Checks if the contract is paused.
fn check_paused(config: &Config) -> Result<(), ContractError> {
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    Ok(())
}

/// Exposes all the queries available in the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => {
            let config = CONFIG.load(deps.storage)?;
            let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;
            let total_balance = deps
                .querier
                .query_balance(&env.contract.address, &config.oro_denom)?
                .amount;
            let total_locked = TOTAL_LOCKED.load(deps.storage)?;
            let effective = total_balance.saturating_sub(Uint128::new(total_locked));

            let exchange_rate = if total_shares.is_zero() || effective.is_zero() {
                Decimal::one()
            } else {
                Decimal::from_ratio(effective, total_shares)
            };

            to_json_binary(&StateResponse {
                total_shares,
                total_deposit: effective,
                exchange_rate,
            })
        }
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::TotalShares {} => {
            let config = CONFIG.load(deps.storage)?;
            let total_supply = deps.querier.query_supply(config.xoro_denom)?.amount;
            to_json_binary(&total_supply)
        }
        QueryMsg::TotalDeposit {} => {
            let config = CONFIG.load(deps.storage)?;
            let total_balance = deps
                .querier
                .query_balance(env.contract.address, config.oro_denom)?
                .amount;
            let total_locked = TOTAL_LOCKED.load(deps.storage)?;
            // Effective deposit subtracts locked unbonding amounts
            let effective = total_balance
                .checked_sub(Uint128::new(total_locked))
                .unwrap_or(Uint128::zero());
            to_json_binary(&effective)
        }
        QueryMsg::BalanceAt { address, timestamp } => {
            if timestamp.is_some() {
                return Err(StdError::generic_err("Historical queries are disabled"));
            }
            let config = CONFIG.load(deps.storage)?;
            let amount = deps.querier
                .query_balance(&address, config.xoro_denom)?
                .amount;
            to_json_binary(&amount)
        }
        QueryMsg::TotalSupplyAt { timestamp } => {
            if timestamp.is_some() {
                return Err(StdError::generic_err("Historical queries are disabled"));
            }
            let config = CONFIG.load(deps.storage)?;
            let amount = deps.querier.query_supply(config.xoro_denom)?.amount;
            to_json_binary(&amount)
        }
        QueryMsg::TotalLocked {} => {
            let total_locked = TOTAL_LOCKED.load(deps.storage)?;
            to_json_binary(&Uint128::new(total_locked))
        }
        QueryMsg::UnbondingEntries { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let entries = UNBONDING
                .may_load(deps.storage, &addr)?
                .unwrap_or_default();
            to_json_binary(&entries)
        }
        QueryMsg::UnbondingPeriod {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&config.unbonding_period)
        }
    }
}
