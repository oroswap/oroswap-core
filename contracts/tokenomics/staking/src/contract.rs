#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal,
    Decimal256, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, StdResult, Storage,
    SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::{must_pay, one_coin};

use oroswap::staking::{
    Config, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse, StakingResponse, UnbondingEntry,
};
use oroswap::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};
use oroswap::token_factory::{
    tf_burn_msg, tf_create_denom_msg, tf_mint_msg,
    MsgCreateDenomResponse,
};
#[cfg(feature = "zigchain")]
use oroswap::token_factory::tf_set_denom_metadata_msg;

use crate::error::ContractError;
use crate::state::{
    CONFIG, OWNERSHIP_PROPOSAL, REWARD_ORO_RESERVE, REWARDS, TOTAL_LOCKED, UNBONDING, USER_REWARDS, RewardInfo,
    UserRewardInfo,
};

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

/// Maximum number of unbonding entries per user
pub const MAX_UNBONDING_ENTRIES: usize = 10;

/// Maximum number of distinct reward denoms
pub const MAX_REWARD_DENOMS: usize = 5;

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
    // Initialize ORO reward reserve to zero
    REWARD_ORO_RESERVE.save(deps.storage, &Uint128::zero())?;

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
            deps.api.addr_validate(&recipient)?;
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
            deps.api.addr_validate(&recipient)?;
            execute_leave(deps, env, info, recipient)
        }
        ExecuteMsg::Claim {} => {
            execute_claim(deps, env, info)
        }
        ExecuteMsg::UpdateConfig { unbonding_period } => {
            execute_update_config(deps, info, unbonding_period)
        }
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;
            propose_new_owner(deps, info, env, owner, expires_in, Addr::unchecked(&config.owner), OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config = CONFIG.load(deps.storage)?;
            drop_ownership_proposal(deps, info, Addr::unchecked(&config.owner), OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.owner = new_owner.to_string();
                    Ok(v)
                })?;
                Ok(())
            })
            .map_err(Into::into)
        }
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Unpause {} => execute_unpause(deps, info),
        ExecuteMsg::AddReward { duration } => execute_add_reward(deps, env, info, duration),
        ExecuteMsg::WithdrawExcessTokens { denom, recipient } => {
            execute_withdraw_excess_tokens(deps, env, info, denom, recipient)
        }
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

                #[cfg(feature = "zigchain")]
                let set_metadata_msg: Option<CosmosMsg> = Some(tf_set_denom_metadata_msg(
                    env.contract.address.clone(),
                    new_token_denom.clone(),
                    TOKEN_SYMBOL.to_string(),
                    config.xoro_decimals,
                    config.xoro_uri.clone(),
                ));
                #[cfg(not(feature = "zigchain"))]
                let set_metadata_msg: Option<CosmosMsg> = None;

                // Bootstrap the pool by minting xORO tokens equal to bootstrap amount
                // Mint to contract itself first, then the ratio is initialized
                let bootstrap_coins = coin(config.bootstrap_amount.u128(), &new_token_denom);
                let bootstrap_mint_msgs = tf_mint_msg(
                    env.contract.address.clone(),
                    bootstrap_coins,
                    env.contract.address.clone(),
                );

                let mut resp = Response::new()
                    .add_messages(bootstrap_mint_msgs)
                    .add_attributes([
                        attr("xoro_denom", new_token_denom),
                        attr("bootstrap_amount", config.bootstrap_amount.to_string()),
                        attr("pool_bootstrapped", "true".to_string()),
                    ]);
                if let Some(m) = set_metadata_msg {
                    resp = resp.add_message(m);
                }
                Ok(resp)
            } else {
                Err(ContractError::FailedToParseReply {})
            }
        }
    }
}

/// Returns the effective ORO balance for ratio calculations:
/// `effective = total_balance - total_locked - reward_oro_reserve`.
/// This prevents unbonding ORO AND ORO rewards from diluting the staker pool.
fn effective_deposit(
    total_balance: Uint128,
    total_locked: u128,
    reward_oro_reserve: Uint128,
) -> Result<Uint128, ContractError> {
    total_balance
        .checked_sub(Uint128::new(total_locked))
        .and_then(|b| b.checked_sub(reward_oro_reserve))
        .map_err(|_| {
            ContractError::Std(StdError::generic_err(
                "total_locked + reward_reserve exceeds oro_balance; state is inconsistent",
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
    let reward_oro_reserve = REWARD_ORO_RESERVE.load(deps.storage)?;

    // Compute effective deposit (excluding user's incoming deposit, which is already in balance)
    let raw_balance = deps
        .querier
        .query_balance(&env.contract.address, &config.oro_denom)?
        .amount;

    // Incoming amount has already landed in contract; subtract it to get pre-deposit balance
    let pre_deposit_balance = raw_balance
        .checked_sub(amount)
        .map_err(|e| ContractError::Std(e.into()))?;
    let effective = effective_deposit(pre_deposit_balance, total_locked, reward_oro_reserve)?;

    let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;
    let global_rewards = update_rewards(deps.storage, &env, total_shares)?;
    // Accrue any rewards earned since the user's last interaction, using their current
    // xORO balance as weight. For a first-time Enter this is zero, so no accrual occurs
    // naturally — no special casing needed.
    let user_xoro_balance = deps
        .querier
        .query_balance(&info.sender, &config.xoro_denom)?
        .amount;
    sync_user_rewards(deps.storage, &info.sender, &global_rewards, user_xoro_balance, false)?;

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
    let reward_oro_reserve = REWARD_ORO_RESERVE.load(deps.storage)?;

    let total_balance = deps
        .querier
        .query_balance(&env.contract.address, &config.oro_denom)?
        .amount;

    let effective = effective_deposit(total_balance, total_locked, reward_oro_reserve)?;
    let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;

    let global_rewards = update_rewards(deps.storage, &env, total_shares)?;
    // Use the user's full pre-leave xORO balance as the reward weight.
    // The xORO being burned has already been transferred to the contract, so the
    // sender's current balance is their remaining shares — add back xoro_amount to
    // get the total shares they held for the accrual period.
    let user_remaining = deps
        .querier
        .query_balance(&info.sender, &config.xoro_denom)?
        .amount;
    let user_total_shares = user_remaining + xoro_amount;
    let accrued_rewards = sync_user_rewards(
        deps.storage,
        &info.sender,
        &global_rewards,
        user_total_shares,
        true,
    )?;

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
        reward_coins: accrued_rewards,
        unlock_time,
        receiver: recipient.clone(),
    };

    let sender = info.sender.clone();
    UNBONDING.update(deps.storage, &sender, |maybe_entries| -> StdResult<_> {
        let mut entries = maybe_entries.unwrap_or_default();
        if entries.len() >= MAX_UNBONDING_ENTRIES {
            return Err(StdError::generic_err(format!(
                "Too many unbonding entries, maximum is {MAX_UNBONDING_ENTRIES}"
            )));
        }
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

    // Update state: keep only pending entries
    if pending.is_empty() {
        UNBONDING.remove(deps.storage, &sender);
    } else {
        UNBONDING.save(deps.storage, &sender, &pending)?;
    }

    // Reduce TOTAL_LOCKED
    let total_claim: Uint128 = matured.iter().map(|e| e.oro_amount).sum();
    TOTAL_LOCKED.update(deps.storage, |locked| -> StdResult<u128> {
        Ok(locked.saturating_sub(total_claim.u128()))
    })?;

    // Group matured entries by receiver and build one BankMsg per unique receiver
    let mut by_receiver: Vec<(String, Vec<cosmwasm_std::Coin>)> = vec![];
    for entry in matured.iter() {
        let pos = by_receiver.iter().position(|(r, _)| r == &entry.receiver);
        let coins = match pos {
            Some(i) => &mut by_receiver[i].1,
            None => {
                by_receiver.push((entry.receiver.clone(), vec![]));
                &mut by_receiver.last_mut().unwrap().1
            }
        };
        // Merge ORO principal
        if let Some(c) = coins.iter_mut().find(|c| c.denom == config.oro_denom) {
            c.amount += entry.oro_amount;
        } else {
            coins.push(coin(entry.oro_amount.u128(), &config.oro_denom));
        }
        // Merge reward coins
        for rc in entry.reward_coins.iter() {
            if let Some(c) = coins.iter_mut().find(|c| c.denom == rc.denom) {
                c.amount += rc.amount;
            } else {
                coins.push(rc.clone());
            }
        }
    }

    // Reduce REWARD_ORO_RESERVE for any ORO rewards being paid out
    let oro_rewards: Uint128 = matured
        .iter()
        .flat_map(|e| e.reward_coins.iter())
        .filter(|c| c.denom == config.oro_denom)
        .map(|c| c.amount)
        .sum();
    if !oro_rewards.is_zero() {
        REWARD_ORO_RESERVE.update(deps.storage, |r| -> StdResult<Uint128> {
            Ok(r.saturating_sub(oro_rewards))
        })?;
    }

    let messages: Vec<CosmosMsg> = by_receiver
        .into_iter()
        .map(|(to_address, amount)| {
            CosmosMsg::from(BankMsg::Send { to_address, amount })
        })
        .collect();

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes([
            attr("action", "claim"),
            attr("sender", &sender),
            attr("oro_claimed", total_claim),
            attr("entries_claimed", matured.len().to_string()),
        ]))
}


/// Updates the contract configuration. Owner only.
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    unbonding_period: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(period) = unbonding_period {
        const MIN_UNBONDING_PERIOD: u64 = 60; // 1 minute minimum
        const MAX_UNBONDING_PERIOD: u64 = 30 * 24 * 60 * 60; // 30 days
        if period < MIN_UNBONDING_PERIOD || period > MAX_UNBONDING_PERIOD {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "unbonding_period must be between {MIN_UNBONDING_PERIOD} and {MAX_UNBONDING_PERIOD} seconds"
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
            let reward_oro_reserve = REWARD_ORO_RESERVE.load(deps.storage)?;
            let effective = total_balance
                .saturating_sub(Uint128::new(total_locked))
                .saturating_sub(reward_oro_reserve);

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
            let reward_oro_reserve = REWARD_ORO_RESERVE.load(deps.storage)?;
            // Effective deposit subtracts locked unbonding amounts and reward reserve
            let effective = total_balance
                .saturating_sub(Uint128::new(total_locked))
                .saturating_sub(reward_oro_reserve);
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
        QueryMsg::PendingRewards { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let config = CONFIG.load(deps.storage)?;
            let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;
            
            let mut rewards = REWARDS.may_load(deps.storage)?.unwrap_or_default();
            let current_ts = env.block.time.seconds();

            // Calculate updated global indexes (read-only)
            for reward in rewards.iter_mut() {
                let last_ts = reward.last_update_ts;
                if last_ts < current_ts {
                    let applicable_end_ts = current_ts.min(reward.end_ts);
                    if applicable_end_ts > last_ts && !total_shares.is_zero() {
                        let time_passed = applicable_end_ts - last_ts;
                        let total_reward = reward.rps * Decimal256::from_ratio(time_passed, 1u64);
                        reward.index += total_reward / Decimal256::from_ratio(total_shares, 1u64);
                    }
                }
            }

            let existing_entry = USER_REWARDS.may_load(deps.storage, &addr)?;
            let is_entered_user = existing_entry.is_some();

            let user_reward_info = existing_entry
                .unwrap_or_else(|| UserRewardInfo {
                    last_rewards_index: vec![],
                    pending_rewards: vec![],
                });

            let mut pending = user_reward_info.pending_rewards.clone();
            // Use the user's actual xORO balance as their weight for the query projection.
            // Also note the config could have been loaded earlier, we skip reloading if possible, but keep it simple here.
            let config = CONFIG.load(deps.storage)?;
            let user_xoro_balance = deps.querier.query_balance(&addr, &config.xoro_denom)?.amount;

            for global_reward in rewards {
                // Consistent with sync_user_rewards: two-tier default logic
                let user_index = user_reward_info
                    .last_rewards_index
                    .iter()
                    .find(|(denom, _)| denom == &global_reward.denom)
                    .map(|(_, index)| *index)
                    .unwrap_or_else(|| {
                        if is_entered_user {
                            Decimal256::zero()
                        } else {
                            global_reward.index
                        }
                    });

                if global_reward.index > user_index && !user_xoro_balance.is_zero() {
                    let diff = global_reward.index - user_index;
                    let pending_amount_u256 = diff * Decimal256::from_ratio(user_xoro_balance, 1u64);
                    let pending_amount: Uint128 = pending_amount_u256.to_uint_floor().try_into()?;

                    if !pending_amount.is_zero() {
                        if let Some(c) = pending.iter_mut().find(|c| c.denom == global_reward.denom) {
                            c.amount += pending_amount;
                        } else {
                            pending.push(coin(pending_amount.u128(), &global_reward.denom));
                        }
                    }
                }
            }
            to_json_binary(&pending)
        }
    }
}

pub fn execute_add_reward(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if duration == 0 {
        return Err(ContractError::InvalidDuration {});
    }

    const MAX_REWARD_DURATION: u64 = 365 * 24 * 60 * 60; // 365 days
    if duration > MAX_REWARD_DURATION {
        return Err(ContractError::InvalidDuration {});
    }

    let reward_coin = one_coin(&info)?;
    let total_shares = deps.querier.query_supply(&config.xoro_denom)?.amount;
    
    let mut global_rewards = update_rewards(deps.storage, &env, total_shares)?;

    let current_ts = env.block.time.seconds();
    let end_ts = current_ts + duration;

    // If the reward token is ORO, track it in REWARD_ORO_RESERVE so it doesn't
    // inflate the ORO:xORO exchange rate.
    if reward_coin.denom == config.oro_denom {
        REWARD_ORO_RESERVE.update(deps.storage, |r| -> StdResult<Uint128> {
            Ok(r + reward_coin.amount)
        })?;
    }

    if let Some(reward) = global_rewards.iter_mut().find(|r| r.denom == reward_coin.denom) {
        let remaining_time = reward.end_ts.saturating_sub(current_ts);
        let remaining_amount = reward.rps * Decimal256::from_ratio(remaining_time, 1u64);
        let new_total_amount = remaining_amount + Decimal256::from_ratio(reward_coin.amount, 1u64);
        
        reward.rps = new_total_amount / Decimal256::from_ratio(duration, 1u64);
        reward.end_ts = end_ts;
    } else {
        if global_rewards.len() >= MAX_REWARD_DENOMS {
            return Err(ContractError::TooManyRewardDenoms { max: MAX_REWARD_DENOMS });
        }
        global_rewards.push(RewardInfo {
            denom: reward_coin.denom.clone(),
            rps: Decimal256::from_ratio(reward_coin.amount, duration),
            index: Decimal256::zero(),
            last_update_ts: current_ts,
            end_ts,
        });
    }

    REWARDS.save(deps.storage, &global_rewards)?;

    Ok(Response::new().add_attributes([
        attr("action", "add_reward"),
        attr("denom", reward_coin.denom),
        attr("amount", reward_coin.amount),
        attr("duration", duration.to_string()),
    ]))
}

fn update_rewards(storage: &mut dyn Storage, env: &Env, total_shares: Uint128) -> StdResult<Vec<RewardInfo>> {
    let mut rewards = REWARDS.may_load(storage)?.unwrap_or_default();
    let current_ts = env.block.time.seconds();

    for reward in rewards.iter_mut() {
        let last_ts = reward.last_update_ts;
        if last_ts >= current_ts {
            continue;
        }

        let applicable_end_ts = current_ts.min(reward.end_ts);
        if applicable_end_ts > last_ts && !total_shares.is_zero() {
            let time_passed = applicable_end_ts - last_ts;
            let total_reward = reward.rps * Decimal256::from_ratio(time_passed, 1u64);
            let index_diff = total_reward / Decimal256::from_ratio(total_shares, 1u64);
            reward.index += index_diff;
        }
        reward.last_update_ts = current_ts;
    }

    REWARDS.save(storage, &rewards)?;
    Ok(rewards)
}
fn execute_withdraw_excess_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender.to_string() != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let to = recipient.unwrap_or_else(|| config.owner.clone());
    deps.api.addr_validate(&to)?;

    let balance = deps
        .querier
        .query_balance(&env.contract.address, &denom)?
        .amount;

    let withdrawable = if denom == config.oro_denom {
        // For ORO, only withdraw the excess above what the contract owes:
        // effective staked pool + locked unbonding + reward reserve.
        // Tokens sent directly inflate the balance without going through Enter,
        // so excess = balance - proper_effective - owed.
        let total_locked = TOTAL_LOCKED.load(deps.storage)?;
        let reward_oro_reserve = REWARD_ORO_RESERVE.load(deps.storage)?;
        let owed = Uint128::new(total_locked) + reward_oro_reserve;
        let proper_effective = effective_deposit(balance, total_locked, reward_oro_reserve)
            .unwrap_or(Uint128::zero());
        balance.saturating_sub(proper_effective + owed)
    } else {
        // Non-ORO tokens not added via AddReward are fully withdrawable
        balance
    };

    if withdrawable.is_zero() {
        return Err(ContractError::Std(StdError::generic_err(
            "No excess tokens to withdraw",
        )));
    }

    let send_msg: CosmosMsg = BankMsg::Send {
        to_address: to.clone(),
        amount: vec![coin(withdrawable.u128(), &denom)],
    }
    .into();

    Ok(Response::new()
        .add_message(send_msg)
        .add_attributes([
            attr("action", "withdraw_excess_tokens"),
            attr("denom", denom),
            attr("amount", withdrawable),
            attr("recipient", to),
        ]))
}


fn sync_user_rewards(
    storage: &mut dyn Storage,
    user: &Addr,
    global_rewards: &[RewardInfo],
    user_shares: Uint128,
    clear_pending: bool,
) -> StdResult<Vec<Coin>> {
    let existing_entry = USER_REWARDS.may_load(storage, user)?;
    // A user who has called Enter at least once will have an entry.
    // A user who received xORO via transfer and never called Enter will have None.
    let is_entered_user = existing_entry.is_some();

    let mut user_reward_info = existing_entry.unwrap_or_else(|| UserRewardInfo {
        last_rewards_index: vec![],
        pending_rewards: vec![],
    });

    for global_reward in global_rewards {
        // Two-tier default logic:
        // - Entered users: unknown new reward denoms default to 0
        //   → they earn from the moment the reward was added (correct, they were already staking)
        // - Non-entered users (transferred xORO): default to CURRENT global index
        //   → they earn nothing retroactively (prevents abuse)
        let user_index = user_reward_info
            .last_rewards_index
            .iter()
            .find(|(denom, _)| denom == &global_reward.denom)
            .map(|(_, index)| *index)
            .unwrap_or_else(|| {
                if is_entered_user {
                    Decimal256::zero()     // existing staker earns from reward start
                } else {
                    global_reward.index   // newcomer (transferred xORO) earns nothing retroactive
                }
            });

        if global_reward.index > user_index && !user_shares.is_zero() {
            let diff = global_reward.index - user_index;
            let pending_amount_u256 = diff * Decimal256::from_ratio(user_shares, 1u64);
            let pending_amount: Uint128 = pending_amount_u256.to_uint_floor().try_into()?;

            if !pending_amount.is_zero() {
                if let Some(c) = user_reward_info
                    .pending_rewards
                    .iter_mut()
                    .find(|c| c.denom == global_reward.denom)
                {
                    c.amount += pending_amount;
                } else {
                    user_reward_info
                        .pending_rewards
                        .push(coin(pending_amount.u128(), &global_reward.denom));
                }
            }
        }

        if let Some(user_reward) = user_reward_info
            .last_rewards_index
            .iter_mut()
            .find(|(denom, _)| denom == &global_reward.denom)
        {
            user_reward.1 = global_reward.index;
        } else {
            user_reward_info
                .last_rewards_index
                .push((global_reward.denom.clone(), global_reward.index));
        }
    }

    let rewards_to_return = user_reward_info.pending_rewards.clone();
    if clear_pending {
        user_reward_info.pending_rewards.clear();
    }

    USER_REWARDS.save(storage, user, &user_reward_info)?;
    Ok(rewards_to_return)
}
