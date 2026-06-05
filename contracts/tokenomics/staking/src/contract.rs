#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg,
    Decimal256, Deps, DepsMut, Env, MessageInfo, Order, Reply, Response, StdError, StdResult,
    Storage, Uint128,
};
use cw2::set_contract_version;
use cw_utils::{must_pay, one_coin};

use oroswap::staking::{
    Config, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse, StakingResponse, UnbondingEntry,
};
use oroswap::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};

use crate::error::ContractError;
use crate::state::{
    CONFIG, FINISHED_REWARDS, ORPHANED_REWARDS, OWNERSHIP_PROPOSAL, PAUSE_AUTHORITIES, REWARDS,
    STAKES, TOTAL_LOCKED, TOTAL_STAKES, UNBONDING, USER_REWARDS, RewardInfo, UserRewardInfo,
};

/// Contract name used for migration.
pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
/// Contract version used for migration.
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum number of unbonding entries per user.
pub const MAX_UNBONDING_ENTRIES: usize = 10;

/// Maximum number of distinct active reward denoms.
pub const MAX_REWARD_DENOMS: usize = 5;

/// Lower bound on `unbonding_period`. Enforced at both `instantiate` and
/// `UpdateConfig` so a misdeployment can't disable the JIT-stake defence
/// (same-block Enter -> Leave -> Claim around a reward event).
pub const MIN_UNBONDING_PERIOD: u64 = 60;

/// Upper bound on `unbonding_period`. Enforced at both `instantiate` and
/// `UpdateConfig`.
pub const MAX_UNBONDING_PERIOD: u64 = 30 * 24 * 60 * 60;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Reject a zero-supply denom up front so we don't ship a contract bricked on first Enter.
    let supply = deps.querier.query_supply(&msg.oro_denom)?.amount;
    ensure!(
        !supply.is_zero(),
        ContractError::Std(StdError::generic_err(
            "oro_denom has 0 supply which is likely sign of misconfiguration"
        ))
    );

    // Owner is taken from the message (not info.sender) so factory-style
    // deployments can hand the contract directly to a multisig.
    let owner = deps.api.addr_validate(&msg.owner)?;

    if msg.minimum_stake_amount.is_zero() {
        return Err(ContractError::MinimumStakeAmountIsZero);
    }

    // Same bounds as UpdateConfig — a deploy-time `0` would defeat the
    // JIT-stake defence around AddReward events.
    if msg.unbonding_period < MIN_UNBONDING_PERIOD
        || msg.unbonding_period > MAX_UNBONDING_PERIOD
    {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "unbonding_period must be between {MIN_UNBONDING_PERIOD} and {MAX_UNBONDING_PERIOD} seconds"
        ))));
    }

    CONFIG.save(
        deps.storage,
        &Config {
            oro_denom: msg.oro_denom.clone(),
            owner: owner.clone(),
            unbonding_period: msg.unbonding_period,
            minimum_stake_amount: msg.minimum_stake_amount,
            paused: false,
        },
    )?;

    TOTAL_LOCKED.save(deps.storage, &0u128)?;
    TOTAL_STAKES.save(deps.storage, &Uint128::zero())?;

    // Pause authorities are NOT seeded at instantiate; the owner adds them
    // post-instantiate via `ExecuteMsg::AddPauseAuthorities` (matches the
    // factory / incentives operator pattern). Owner retains implicit
    // pause authority and is intentionally not stored in PAUSE_AUTHORITIES.

    Ok(Response::new().add_attributes(vec![
        attr("action", "instantiate"),
        attr("contract", CONTRACT_NAME),
        attr("oro_denom", msg.oro_denom),
        attr("unbonding_period", msg.unbonding_period.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Enter {} => execute_enter(deps, env, info),
        ExecuteMsg::Leave { amount, recipient } => {
            deps.api.addr_validate(&recipient)?;
            execute_leave(deps, env, info, amount, recipient)
        }
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::AddReward { duration } => execute_add_reward(deps, env, info, duration),
        ExecuteMsg::UpdateConfig {
            unbonding_period,
            minimum_stake_amount,
        } => execute_update_config(deps, info, unbonding_period, minimum_stake_amount),
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
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Unpause {} => execute_unpause(deps, info),
        ExecuteMsg::WithdrawExcessTokens { denom, recipient } => {
            execute_withdraw_excess_tokens(deps, env, info, denom, recipient)
        }
        ExecuteMsg::AddPauseAuthorities { addresses } => {
            execute_add_pause_authorities(deps, info, addresses)
        }
        ExecuteMsg::RemovePauseAuthorities { addresses } => {
            execute_remove_pause_authorities(deps, info, addresses)
        }
        ExecuteMsg::ClaimOrphanedRewards { recipient } => {
            execute_claim_orphaned_rewards(deps, env, info, recipient)
        }
    }
}

/// No sub-messages are dispatched, so any reply is an unknown id.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, _msg: Reply) -> Result<Response, ContractError> {
    Err(ContractError::FailedToParseReply {})
}

fn execute_enter(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    let amount = must_pay(&info, &config.oro_denom)?;

    ensure!(
        amount >= config.minimum_stake_amount,
        ContractError::AmountBelowMinimum {
            minimum: config.minimum_stake_amount,
            provided: amount,
        }
    );

    let total_stakes = TOTAL_STAKES.load(deps.storage)?;

    // Sync rewards against the sender's CURRENT stake balance BEFORE mutating
    // it — otherwise this Enter's amount would back-credit prior accrual.
    let user_stakes = STAKES
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    let global_rewards = update_rewards(deps.storage, &env, total_stakes)?;
    sync_user_rewards(deps.storage, &info.sender, &global_rewards, user_stakes, false)?;

    STAKES.update(deps.storage, &info.sender, |s| -> StdResult<_> {
        Ok(s.unwrap_or_default().checked_add(amount)?)
    })?;
    TOTAL_STAKES.update(deps.storage, |t| -> StdResult<_> {
        Ok(t.checked_add(amount)?)
    })?;

    let staking_response = to_json_binary(&StakingResponse {
        oro_amount: amount,
        share_amount: amount,
    })?;

    Ok(Response::new()
        .set_data(staking_response)
        .add_attributes([
            attr("action", "enter"),
            attr("sender", &info.sender),
            attr("oro_amount", amount),
            attr("share_amount", amount),
        ]))
}

fn execute_leave(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    if amount.is_zero() {
        return Err(ContractError::StakeAmountTooSmall {});
    }

    let user_stakes = STAKES
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    ensure!(
        amount <= user_stakes,
        ContractError::InsufficientStake {
            available: user_stakes,
            requested: amount,
        }
    );

    // Full exits (amount == user_stakes) always bypass the minimum: if the
    // owner raised the floor after a user staked, they must still be able to
    // exit, and we never strand sub-minimum dust. No "dust remaining" check.
    let is_full_exit = amount == user_stakes;
    ensure!(
        is_full_exit || amount >= config.minimum_stake_amount,
        ContractError::AmountBelowMinimum {
            minimum: config.minimum_stake_amount,
            provided: amount,
        }
    );

    let total_stakes = TOTAL_STAKES.load(deps.storage)?;

    // Settle accrual against the FULL pre-debit stake. `clear_pending = true`
    // empties the pending bucket so a subsequent Leave-before-Claim cannot
    // pay the same accrual twice.
    let global_rewards = update_rewards(deps.storage, &env, total_stakes)?;
    let accrued_rewards = sync_user_rewards(
        deps.storage,
        &info.sender,
        &global_rewards,
        user_stakes,
        true,
    )?;

    let return_amount = amount;

    STAKES.update(deps.storage, &info.sender, |s| -> StdResult<_> {
        Ok(s.unwrap_or_default().checked_sub(amount)?)
    })?;
    TOTAL_STAKES.update(deps.storage, |t| -> StdResult<_> {
        Ok(t.checked_sub(amount)?)
    })?;

    let unlock_time = env.block.time.seconds() + config.unbonding_period;
    let entry = UnbondingEntry {
        oro_amount: return_amount,
        reward_coins: accrued_rewards,
        unlock_time,
        receiver: recipient.clone(),
    };

    let sender = info.sender.clone();
    // Length check outside the .update closure so the typed
    // TooManyUnbondingEntries error escapes instead of a stringly StdError.
    let mut entries = UNBONDING
        .may_load(deps.storage, &sender)?
        .unwrap_or_default();
    if entries.len() >= MAX_UNBONDING_ENTRIES {
        return Err(ContractError::TooManyUnbondingEntries {
            max: MAX_UNBONDING_ENTRIES,
        });
    }
    entries.push(entry);
    UNBONDING.save(deps.storage, &sender, &entries)?;

    TOTAL_LOCKED.update(deps.storage, |locked| -> StdResult<u128> {
        Ok(locked + return_amount.u128())
    })?;

    let staking_response = to_json_binary(&StakingResponse {
        oro_amount: return_amount,
        share_amount: amount,
    })?;

    Ok(Response::new()
        .set_data(staking_response)
        .add_attributes([
            attr("action", "leave"),
            attr("sender", &sender),
            attr("recipient", &recipient),
            attr("share_amount", amount),
            attr("oro_locked", return_amount),
            attr("unlock_time", unlock_time.to_string()),
        ]))
}

/// Release all matured unbonding entries to the receivers recorded at Leave time.
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

    if pending.is_empty() {
        UNBONDING.remove(deps.storage, &sender);
    } else {
        UNBONDING.save(deps.storage, &sender, &pending)?;
    }

    let total_claim: Uint128 = matured.iter().map(|e| e.oro_amount).sum();
    TOTAL_LOCKED.update(deps.storage, |locked| -> StdResult<u128> {
        Ok(locked.saturating_sub(total_claim.u128()))
    })?;

    // Group matured entries by receiver: one BankMsg per unique destination
    // with merged ORO + reward coins.
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
        if let Some(c) = coins.iter_mut().find(|c| c.denom == config.oro_denom) {
            c.amount += entry.oro_amount;
        } else {
            coins.push(coin(entry.oro_amount.u128(), &config.oro_denom));
        }
        for rc in entry.reward_coins.iter() {
            if let Some(c) = coins.iter_mut().find(|c| c.denom == rc.denom) {
                c.amount += rc.amount;
            } else {
                coins.push(rc.clone());
            }
        }
    }

    let messages: Vec<CosmosMsg> = by_receiver
        .into_iter()
        .map(|(to_address, amount)| CosmosMsg::from(BankMsg::Send { to_address, amount }))
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

/// Update mutable config fields. Owner-only; ownership rotation goes through
/// the two-step `ProposeNewOwner` + `ClaimOwnership` flow.
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    unbonding_period: Option<u64>,
    minimum_stake_amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(period) = unbonding_period {
        if period < MIN_UNBONDING_PERIOD || period > MAX_UNBONDING_PERIOD {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "unbonding_period must be between {MIN_UNBONDING_PERIOD} and {MAX_UNBONDING_PERIOD} seconds"
            ))));
        }
        config.unbonding_period = period;
    }

    if let Some(min) = minimum_stake_amount {
        if min.is_zero() {
            return Err(ContractError::MinimumStakeAmountIsZero);
        }
        config.minimum_stake_amount = min;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Pause the contract. Callable by the owner or any pause-authority.
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

/// Unpause the contract. Owner-only — keeps a compromised pause-authority
/// from being able to toggle pause state to grief.
fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::NoUnpauseAuthority {});
    }

    config.paused = false;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "unpause"))
}

/// Passes if `sender` is the owner or a registered pause authority.
fn check_pause_authority(
    storage: &dyn Storage,
    sender: &Addr,
) -> Result<(), ContractError> {
    let config = CONFIG.load(storage)?;
    if sender == &config.owner {
        return Ok(());
    }
    if PAUSE_AUTHORITIES.has(storage, sender) {
        return Ok(());
    }
    Err(ContractError::Unauthorized {})
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

fn check_paused(config: &Config) -> Result<(), ContractError> {
    if config.paused {
        return Err(ContractError::ContractPaused {});
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => {
            let total_stakes = TOTAL_STAKES.load(deps.storage)?;
            // 1:1 model: total_deposit == total_shares == TOTAL_STAKES.
            to_json_binary(&StateResponse {
                total_shares: total_stakes,
                total_deposit: total_stakes,
            })
        }
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::StakeOf { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let amount = STAKES.may_load(deps.storage, &addr)?.unwrap_or_default();
            to_json_binary(&amount)
        }
        QueryMsg::TotalShares {} => to_json_binary(&TOTAL_STAKES.load(deps.storage)?),
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
            let total_stakes = TOTAL_STAKES.load(deps.storage)?;

            let mut rewards = REWARDS.may_load(deps.storage)?.unwrap_or_default();
            let current_ts = env.block.time.seconds();

            // Read-only mirror of update_rewards so the query reflects the
            // accrual the next mutation would commit.
            for reward in rewards.iter_mut() {
                let last_ts = reward.last_update_ts;
                if last_ts < current_ts {
                    let applicable_end_ts = current_ts.min(reward.end_ts);
                    if applicable_end_ts > last_ts && !total_stakes.is_zero() {
                        let time_passed = applicable_end_ts - last_ts;
                        let total_reward =
                            reward.rps * Decimal256::from_ratio(time_passed, 1u64);
                        reward.index +=
                            total_reward / Decimal256::from_ratio(total_stakes, 1u64);
                    }
                }
            }

            let user_reward_info = USER_REWARDS
                .may_load(deps.storage, &addr)?
                .unwrap_or_else(|| UserRewardInfo {
                    last_rewards_index: vec![],
                    pending_rewards: vec![],
                });

            // Build a Decimal256 accumulator seeded with the carried fractional
            // remainder, mirror update_rewards + sync_user_rewards into it, then
            // floor each denom ONCE before returning. The UI gets clean integer
            // Coins; the contract never persists this read-only state.
            let mut acc: Vec<(String, Decimal256)> = user_reward_info.pending_rewards.clone();
            let user_shares = STAKES.may_load(deps.storage, &addr)?.unwrap_or_default();

            // Pass 1: accrual from currently-active streams.
            let live_denoms: std::collections::HashSet<String> =
                rewards.iter().map(|r| r.denom.clone()).collect();
            for global_reward in &rewards {
                let user_index = user_reward_info
                    .last_rewards_index
                    .iter()
                    .find(|(denom, _)| denom == &global_reward.denom)
                    .map(|(_, index)| *index)
                    .unwrap_or(Decimal256::zero());

                if global_reward.index > user_index && !user_shares.is_zero() {
                    let diff = global_reward.index - user_index;
                    let accrual = diff * Decimal256::from_ratio(user_shares, 1u64);
                    if let Some(c) = acc.iter_mut().find(|(d, _)| d == &global_reward.denom) {
                        c.1 += accrual;
                    } else {
                        acc.push((global_reward.denom.clone(), accrual));
                    }
                }
            }

            // FIND-015 Pass 2: read-only mirror — include rewards from streams
            // pruned into FINISHED_REWARDS.
            for entry in FINISHED_REWARDS.range(deps.storage, None, None, Order::Ascending) {
                let (denom, finished_index) = entry?;
                if live_denoms.contains(&denom) {
                    continue;
                }

                let user_index = user_reward_info
                    .last_rewards_index
                    .iter()
                    .find(|(d, _)| d == &denom)
                    .map(|(_, idx)| *idx)
                    .unwrap_or(Decimal256::zero());

                if finished_index > user_index && !user_shares.is_zero() {
                    let diff = finished_index - user_index;
                    let accrual = diff * Decimal256::from_ratio(user_shares, 1u64);
                    if let Some(c) = acc.iter_mut().find(|(d, _)| d == &denom) {
                        c.1 += accrual;
                    } else {
                        acc.push((denom, accrual));
                    }
                }
            }

            // Floor at the API boundary; suppress zero-amount entries so the
            // UI never has to filter them out.
            let pending: Vec<Coin> = acc
                .into_iter()
                .filter_map(|(denom, d)| {
                    let amount: Uint128 = d.to_uint_floor().try_into().ok()?;
                    if amount.is_zero() {
                        None
                    } else {
                        Some(coin(amount.u128(), denom.as_str()))
                    }
                })
                .collect();
            to_json_binary(&pending)
        }
        QueryMsg::PauseAuthorities {} => {
            let authorities: Vec<String> = PAUSE_AUTHORITIES
                .keys(deps.storage, None, None, Order::Ascending)
                .filter_map(|res| res.ok().map(|addr| addr.into_string()))
                .collect();
            to_json_binary(&authorities)
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
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if duration == 0 {
        return Err(ContractError::InvalidDuration {});
    }

    const MAX_REWARD_DURATION: u64 = 365 * 24 * 60 * 60;
    if duration > MAX_REWARD_DURATION {
        return Err(ContractError::InvalidDuration {});
    }

    let reward_coin = one_coin(&info)?;
    let total_stakes = TOTAL_STAKES.load(deps.storage)?;

    let mut global_rewards = update_rewards(deps.storage, &env, total_stakes)?;

    let current_ts = env.block.time.seconds();
    let end_ts = current_ts + duration;

    if let Some(reward) = global_rewards
        .iter_mut()
        .find(|r| r.denom == reward_coin.denom)
    {
        let remaining_time = reward.end_ts.saturating_sub(current_ts);
        let remaining_amount = reward.rps * Decimal256::from_ratio(remaining_time, 1u64);
        let new_total_amount = remaining_amount + Decimal256::from_ratio(reward_coin.amount, 1u64);

        reward.rps = new_total_amount / Decimal256::from_ratio(duration, 1u64);
        reward.end_ts = end_ts;
    } else {
        // FIND-015: cap is checked against the POST-PRUNE active list so a
        // just-expired stream frees its slot.
        if global_rewards.len() >= MAX_REWARD_DENOMS {
            return Err(ContractError::TooManyRewardDenoms {
                max: MAX_REWARD_DENOMS,
            });
        }
        // FIND-015: resume from the prior finished index (not zero) so users
        // whose snapshot still holds it don't see `global < user` and miss
        // the prior stream's accrual. Also drop the FINISHED_REWARDS entry to
        // avoid double-counting via the Pass-2 fallback in sync_user_rewards.
        let starting_index = FINISHED_REWARDS
            .may_load(deps.storage, reward_coin.denom.as_str())?
            .unwrap_or(Decimal256::zero());
        if !starting_index.is_zero() {
            FINISHED_REWARDS.remove(deps.storage, reward_coin.denom.as_str());
        }
        global_rewards.push(RewardInfo {
            denom: reward_coin.denom.clone(),
            rps: Decimal256::from_ratio(reward_coin.amount, duration),
            index: starting_index,
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

fn update_rewards(
    storage: &mut dyn Storage,
    env: &Env,
    total_shares: Uint128,
) -> StdResult<Vec<RewardInfo>> {
    let mut rewards = REWARDS.may_load(storage)?.unwrap_or_default();
    let current_ts = env.block.time.seconds();

    // Pass 1: accrue each active stream up to min(now, end_ts). Once past
    // end_ts, the index is final and never changes again.
    //
    // When `total_shares == 0` the emission cannot be distributed via the
    // index (divide-by-zero / unclaimable), so it is parked in
    // ORPHANED_REWARDS instead. `last_update_ts` MUST advance in BOTH
    // branches or the same window would be re-emitted to whichever staker
    // enters next, breaking the solvency invariant.
    //
    // Rounding: orphan accrual is floored to Uint128. With N concurrent
    // reward denoms up to (N - 1) wei may be lost per orphan event; the
    // dust stays in contract balance and never under/over-credits a staker.
    for reward in rewards.iter_mut() {
        let last_ts = reward.last_update_ts;
        if last_ts >= current_ts {
            continue;
        }

        let applicable_end_ts = current_ts.min(reward.end_ts);
        if applicable_end_ts <= last_ts {
            // Stream ended before last_ts — still advance last_update_ts so
            // we don't revisit on subsequent calls.
            reward.last_update_ts = current_ts;
            continue;
        }

        let time_passed = applicable_end_ts - last_ts;
        let total_reward = reward.rps * Decimal256::from_ratio(time_passed, 1u64);

        if total_shares.is_zero() {
            let orphan_floor: Uint128 = total_reward.to_uint_floor().try_into()?;
            if !orphan_floor.is_zero() {
                ORPHANED_REWARDS.update(
                    storage,
                    reward.denom.as_str(),
                    |existing| -> StdResult<_> {
                        Ok(existing.unwrap_or_default().checked_add(orphan_floor)?)
                    },
                )?;
            }
        } else {
            let index_diff = total_reward / Decimal256::from_ratio(total_shares, 1u64);
            reward.index += index_diff;
        }

        reward.last_update_ts = applicable_end_ts;
    }

    // Pass 2 (FIND-015): lazy-prune fully-elapsed streams into
    // FINISHED_REWARDS so sync_user_rewards / PendingRewards can still credit
    // the `(final_index - snapshot) * shares` owed to users who hadn't synced
    // before the prune.
    let mut still_active = Vec::with_capacity(rewards.len());
    for reward in rewards.into_iter() {
        if current_ts >= reward.end_ts {
            FINISHED_REWARDS.save(storage, reward.denom.as_str(), &reward.index)?;
        } else {
            still_active.push(reward);
        }
    }

    REWARDS.save(storage, &still_active)?;
    Ok(still_active)
}

/// Recover non-ORO, non-reward-stream tokens accidentally sent to the contract.
/// Owner only. ORO is non-recoverable because every wei must remain available
/// to honour Enter/Leave/Claim at the 1:1 invariant; reward-stream denoms
/// (live or finished) are non-recoverable because they back accrued-but-
/// unclaimed liabilities in USER_REWARDS and pending UnbondingEntry.reward_coins.
fn execute_withdraw_excess_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if denom == config.oro_denom {
        return Err(ContractError::OroNonRecoverable {});
    }

    let rewards = REWARDS.may_load(deps.storage)?.unwrap_or_default();
    if rewards.iter().any(|r| r.denom == denom) {
        return Err(ContractError::DenomInRewardStream { denom });
    }
    if FINISHED_REWARDS.has(deps.storage, denom.as_str()) {
        return Err(ContractError::DenomInRewardStream { denom });
    }

    let to = recipient.unwrap_or_else(|| config.owner.to_string());
    deps.api.addr_validate(&to)?;

    let balance = deps
        .querier
        .query_balance(&env.contract.address, &denom)?
        .amount;

    if balance.is_zero() {
        return Err(ContractError::NoExcessToWithdraw {});
    }

    let send_msg: CosmosMsg = BankMsg::Send {
        to_address: to.clone(),
        amount: vec![coin(balance.u128(), &denom)],
    }
    .into();

    Ok(Response::new()
        .add_message(send_msg)
        .add_attributes([
            attr("action", "withdraw_excess_tokens"),
            attr("denom", denom),
            attr("amount", balance),
            attr("recipient", to),
        ]))
}

/// Sweep all orphaned reward tokens (emissions accrued while
/// `TOTAL_STAKES == 0`) to `recipient` or the owner. Owner only.
fn execute_claim_orphaned_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_paused(&config)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let to_addr = match recipient {
        Some(raw) => deps.api.addr_validate(&raw)?,
        None => config.owner.clone(),
    };

    // Filter zero rows defensively against any future migration leaving a
    // legacy zero entry — we never want to emit a zero-coin in BankMsg.
    let entries: Vec<(String, Uint128)> = ORPHANED_REWARDS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    if entries.is_empty() {
        return Err(ContractError::NoOrphanedRewards);
    }

    let mut coins_out: Vec<Coin> = Vec::with_capacity(entries.len());
    for (denom, amount) in entries.iter() {
        if !amount.is_zero() {
            coins_out.push(coin(amount.u128(), denom));
        }
    }
    if coins_out.is_empty() {
        return Err(ContractError::NoOrphanedRewards);
    }

    // Clear keys AFTER the empty check so the error path doesn't partially wipe.
    for (denom, _) in entries.iter() {
        ORPHANED_REWARDS.remove(deps.storage, denom.as_str());
    }

    // BankMsg::Send requires denom-sorted coins. ORPHANED_REWARDS is iterated
    // in ascending key order and we preserve that order here.
    let send_msg: CosmosMsg = BankMsg::Send {
        to_address: to_addr.to_string(),
        amount: coins_out.clone(),
    }
    .into();

    let amounts_attr = coins_out
        .iter()
        .map(|c| format!("{}:{}", c.denom, c.amount))
        .collect::<Vec<_>>()
        .join(",");

    Ok(Response::new().add_message(send_msg).add_attributes([
        attr("action", "claim_orphaned_rewards"),
        attr("recipient", to_addr.to_string()),
        attr("amounts", amounts_attr),
    ]))
}

/// Accrue rewards earned since the user's last interaction. A user with no
/// snapshot for a denom starts from `Decimal256::zero()`.
///
/// FIND-dust: accrual is accumulated as Decimal256 (no per-sync floor) and
/// flooring only happens at the boundary when `clear_pending=true` materialises
/// the bucket into integer Coin(s). The sub-unit remainder stays in
/// `pending_rewards` so it can grow past 1 unit on a future sync — preventing
/// the "interact-every-block burns rewards" rounding loss.
fn sync_user_rewards(
    storage: &mut dyn Storage,
    user: &Addr,
    global_rewards: &[RewardInfo],
    user_shares: Uint128,
    clear_pending: bool,
) -> StdResult<Vec<Coin>> {
    let mut user_reward_info = USER_REWARDS
        .may_load(storage, user)?
        .unwrap_or_else(|| UserRewardInfo {
            last_rewards_index: vec![],
            pending_rewards: vec![],
        });

    for global_reward in global_rewards {
        let user_index = user_reward_info
            .last_rewards_index
            .iter()
            .find(|(denom, _)| denom == &global_reward.denom)
            .map(|(_, index)| *index)
            .unwrap_or(Decimal256::zero());

        if global_reward.index > user_index && !user_shares.is_zero() {
            let diff = global_reward.index - user_index;
            let accrual = diff * Decimal256::from_ratio(user_shares, 1u64);

            if let Some(c) = user_reward_info
                .pending_rewards
                .iter_mut()
                .find(|(d, _)| d == &global_reward.denom)
            {
                c.1 += accrual;
            } else {
                user_reward_info
                    .pending_rewards
                    .push((global_reward.denom.clone(), accrual));
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

    // FIND-015 Pass 2: settle accrual from streams pruned out of REWARDS.
    // Iterates FINISHED_REWARDS (not the user's snapshot) so a user who
    // staked before the reward was added and never re-synced before expiry
    // still gets credited. Skips any denom still in the live list.
    let live_denoms: std::collections::HashSet<String> =
        global_rewards.iter().map(|r| r.denom.clone()).collect();
    let finished_entries: Vec<(String, Decimal256)> = FINISHED_REWARDS
        .range(storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;
    for (denom, finished_index) in finished_entries {
        if live_denoms.contains(&denom) {
            continue;
        }

        let user_index = user_reward_info
            .last_rewards_index
            .iter()
            .find(|(d, _)| d == &denom)
            .map(|(_, idx)| *idx)
            .unwrap_or(Decimal256::zero());

        if finished_index > user_index && !user_shares.is_zero() {
            let diff = finished_index - user_index;
            let accrual = diff * Decimal256::from_ratio(user_shares, 1u64);
            if let Some(c) = user_reward_info
                .pending_rewards
                .iter_mut()
                .find(|(d, _)| d == &denom)
            {
                c.1 += accrual;
            } else {
                user_reward_info
                    .pending_rewards
                    .push((denom.clone(), accrual));
            }
        }

        // Snap user index to the finished value so subsequent syncs compute
        // `diff = 0` and don't re-credit.
        if let Some(snap) = user_reward_info
            .last_rewards_index
            .iter_mut()
            .find(|(d, _)| d == &denom)
        {
            snap.1 = finished_index;
        } else {
            user_reward_info
                .last_rewards_index
                .push((denom, finished_index));
        }
    }

    // FIND-dust: when called from Leave (clear_pending=true) materialise the
    // decimal pending bucket into integer Coins, then CARRY the sub-1
    // remainder back into pending_rewards so the next sync builds on today's
    // leftovers instead of starting from zero.
    let rewards_to_return: Vec<Coin> = if clear_pending {
        let mut out: Vec<Coin> = Vec::with_capacity(user_reward_info.pending_rewards.len());
        for (denom, decimal_amount) in user_reward_info.pending_rewards.iter_mut() {
            let payout: Uint128 = decimal_amount.to_uint_floor().try_into()?;
            if !payout.is_zero() {
                out.push(coin(payout.u128(), denom.as_str()));
                *decimal_amount -= Decimal256::from_ratio(payout, 1u64);
            }
        }
        // Drop fully-zero entries to keep the vec compact.
        user_reward_info
            .pending_rewards
            .retain(|(_, d)| !d.is_zero());
        out
    } else {
        // Callers (Enter) ignore this return value; return empty for cheapness.
        vec![]
    };

    USER_REWARDS.save(storage, user, &user_reward_info)?;
    Ok(rewards_to_return)
}
