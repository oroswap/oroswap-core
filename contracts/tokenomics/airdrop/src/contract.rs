use std::collections::BTreeMap;

use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult, Uint128, WasmMsg, StdError,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use cw_utils::must_pay;
use oroswap::common::{propose_new_owner, drop_ownership_proposal, claim_ownership};

use crate::error::ContractError;
use crate::msg::{
    AirdropResponse, ConfigResponse, ExecuteMsg,
    InstantiateMsg, LatestAirdropIdResponse, QueryMsg, SolvencyResponse, UserAirdropStatusResponse,
};
use crate::state::{
    AirdropInfo, Config, UserClaim, AIRDROPS, CLAIMED_TOTAL, CONFIG, ELIGIBILITY,
    ELIGIBILITY_TOTAL, LATEST_AIRDROP_ID, OWNERSHIP_PROPOSAL, USER_CLAIMS,
};

pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Hard cap on the number of `(address, amount)` entries that `AddAirdrop`
/// or `UpdateEligibility` can accept in a single tx. Large airdrops must be
/// split across multiple `UpdateEligibility` calls so a single message never
/// exceeds the chain's block-gas / message-size limits.
pub const MAX_ELIGIBILITY_BATCH: usize = 200;

/// Hard cap on the APY-style cliff bonus, in basis points. 10_000 bps =
/// 100% — sanity bound, mostly to prevent a misconfigured admin call
/// from committing more tokens than the contract can hold.
pub const MAX_APY_BONUS_BPS: u64 = 10_000;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
    };
    CONFIG.save(deps.storage, &config)?;
    LATEST_AIRDROP_ID.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", msg.admin))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddAirdrop {
            token_address,
            is_native,
            start_time,
            vesting_start_time,
            vesting_end_time,
            initial_claim_percent,
            apy_bonus_bps,
            eligibility,
            expiry_time,
        } => execute_add_airdrop(
            deps,
            env,
            info,
            token_address,
            is_native,
            start_time,
            vesting_start_time,
            vesting_end_time,
            initial_claim_percent,
            apy_bonus_bps,
            eligibility,
            expiry_time,
        ),
        ExecuteMsg::ToggleAirdrop { id, active } => {
            execute_toggle_airdrop(deps, info, id, active)
        }
        ExecuteMsg::Claim { airdrop_id } => execute_claim(deps, env, info, airdrop_id),
        ExecuteMsg::DepositFunds { token_address, amount, is_native } => execute_deposit_funds(deps, env, info, token_address, amount, is_native),
        ExecuteMsg::WithdrawFunds { token_address, amount, is_native } => execute_withdraw_funds(deps, env, info, token_address, amount, is_native),
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;
            propose_new_owner(deps, info, env, owner, expires_in, config.admin, OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config = CONFIG.load(deps.storage)?;
            drop_ownership_proposal(deps, info, config.admin, OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG.update::<_, StdError>(deps.storage, |mut v| {
                    v.admin = new_owner;
                    Ok(v)
                })?;
                Ok(())
            })
            .map_err(Into::into)
        }
        ExecuteMsg::UpdateEligibility { airdrop_id, eligibility } => execute_update_eligibility(deps, env, info, airdrop_id, eligibility)
    }
}

pub fn execute_add_airdrop(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_address: String,
    is_native: bool,
    start_time: u64,
    vesting_start_time: u64,
    vesting_end_time: u64,
    initial_claim_percent: u64,
    apy_bonus_bps: u64,
    eligibility: Vec<(String, Uint128)>,
    expiry_time: Option<u64>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if vesting_start_time < start_time || vesting_start_time >= vesting_end_time {
        return Err(ContractError::InvalidVestingWindow {});
    }

    if initial_claim_percent > 100 {
        return Err(ContractError::InvalidInitialPercent {
            got: initial_claim_percent,
        });
    }

    if apy_bonus_bps > MAX_APY_BONUS_BPS {
        return Err(ContractError::ApyBonusTooHigh {
            got: apy_bonus_bps,
            max: MAX_APY_BONUS_BPS,
        });
    }

    if start_time < env.block.time.seconds() {
        return Err(ContractError::StartTimeInPast {});
    }

    if let Some(expiry) = expiry_time {
        if expiry <= vesting_end_time {
            return Err(ContractError::InvalidExpiry {});
        }
    }

    if eligibility.len() > MAX_ELIGIBILITY_BATCH {
        return Err(ContractError::EligibilityBatchTooLarge {
            got: eligibility.len(),
            max: MAX_ELIGIBILITY_BATCH,
        });
    }

    let airdrop_id = LATEST_AIRDROP_ID.load(deps.storage)? + 1;
    LATEST_AIRDROP_ID.save(deps.storage, &airdrop_id)?;

    AIRDROPS.save(
        deps.storage,
        airdrop_id,
        &AirdropInfo {
            id: airdrop_id,
            token_address: token_address.clone(),
            start_time,
            vesting_start_time,
            vesting_end_time,
            initial_claim_percent,
            apy_bonus_bps,
            is_active: true,
            is_native,
            expiry_time,
        },
    )?;

    // Dedupe the input batch in-memory (last-wins per address) so a caller
    // passing the same address twice doesn't inflate ELIGIBILITY_TOTAL above
    // what actually lands in storage. BTreeMap keeps iteration deterministic
    // for CosmWasm consensus; no storage reads needed since ELIGIBILITY is
    // fresh for a new airdrop_id — duplicates can only come from this batch.
    let mut deduped: BTreeMap<cosmwasm_std::Addr, Uint128> = BTreeMap::new();
    for (addr, amount) in eligibility {
        let validated_addr = deps.api.addr_validate(&addr)?;
        deduped.insert(validated_addr, amount);
    }

    let mut total = Uint128::zero();
    for (addr, amount) in deduped {
        ELIGIBILITY.save(deps.storage, (airdrop_id, &addr), &amount)?;
        total = total.checked_add(amount).map_err(StdError::from)?;
    }
    ELIGIBILITY_TOTAL.save(deps.storage, airdrop_id, &total)?;
    CLAIMED_TOTAL.save(deps.storage, airdrop_id, &Uint128::zero())?;

    Ok(Response::new()
        .add_attribute("action", "add_airdrop")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("token_address", token_address)
        .add_attribute("total_eligibility", total))
}

pub fn execute_toggle_airdrop(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
    active: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let mut airdrop = AIRDROPS
        .may_load(deps.storage, id)?
        .ok_or(ContractError::AirdropNotFound {})?;

    airdrop.is_active = active;
    AIRDROPS.save(deps.storage, id, &airdrop)?;

    Ok(Response::new()
        .add_attribute("action", "toggle_airdrop")
        .add_attribute("id", id.to_string())
        .add_attribute("active", active.to_string()))
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    airdrop_id: u64,
) -> Result<Response, ContractError> {
    let airdrop = AIRDROPS
        .may_load(deps.storage, airdrop_id)?
        .ok_or(ContractError::AirdropNotFound {})?;

    let current_time = env.block.time.seconds();

    if current_time < airdrop.start_time {
        return Err(ContractError::AirdropNotStarted {});
    }

    if !airdrop.is_active {
        return Err(ContractError::AirdropInactive {});
    }

    if let Some(expiry) = airdrop.expiry_time {
        if current_time >= expiry {
            return Err(ContractError::AirdropExpired {});
        }
    }

    // Single claim check
    if USER_CLAIMS.has(deps.storage, (airdrop_id, &info.sender)) {
        return Err(ContractError::AlreadyClaimed {});
    }

    let amount_eligible = ELIGIBILITY
        .may_load(deps.storage, (airdrop_id, &info.sender))?
        .ok_or(ContractError::NotEligible {})?;

    if amount_eligible.is_zero() {
        return Err(ContractError::NotEligible {});
    }

    // 1. Initial Portion (e.g. 40%)
    let initial_payout = amount_eligible.multiply_ratio(airdrop.initial_claim_percent, 100u128);

    // 2. Vesting portion (60% of original)
    let vesting_base = amount_eligible.saturating_sub(initial_payout);
    
    let mut unlocked_vesting = Uint128::zero();
    let mut bonus = Uint128::zero();

    if current_time > airdrop.vesting_start_time {
        let elapsed = current_time.min(airdrop.vesting_end_time).saturating_sub(airdrop.vesting_start_time);
        let duration = airdrop.vesting_end_time.saturating_sub(airdrop.vesting_start_time);

        if duration > 0 {
            unlocked_vesting = vesting_base.multiply_ratio(elapsed, duration);
            
            // 3. APY Bonus - ONLY if claiming at or after vesting_end_time
            if current_time >= airdrop.vesting_end_time {
                bonus = vesting_base.multiply_ratio(airdrop.apy_bonus_bps, 10000u64);
            }
        }
    }

    let total_payout = initial_payout.checked_add(unlocked_vesting).map_err(StdError::from)?
                                   .checked_add(bonus).map_err(StdError::from)?;

    USER_CLAIMS.save(
        deps.storage,
        (airdrop_id, &info.sender),
        &UserClaim {
            amount_eligible,
            amount_claimed: total_payout,
            claim_time: current_time,
        },
    )?;

    // Update running claimed total so the Solvency query stays O(1).
    let prior_total = CLAIMED_TOTAL
        .may_load(deps.storage, airdrop_id)?
        .unwrap_or_default();
    CLAIMED_TOTAL.save(
        deps.storage,
        airdrop_id,
        &prior_total
            .checked_add(total_payout)
            .map_err(StdError::from)?,
    )?;

    let mut messages: Vec<CosmosMsg> = vec![];
    if !total_payout.is_zero() {
        let msg: CosmosMsg = if airdrop.is_native {
            cosmwasm_std::BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![cosmwasm_std::Coin {
                    denom: airdrop.token_address,
                    amount: total_payout,
                }],
            }.into()
        } else {
            WasmMsg::Execute {
                contract_addr: airdrop.token_address,
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: total_payout,
                })?,
                funds: vec![],
            }.into()
        };
        messages.push(msg);
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("user", info.sender)
        .add_attribute("total_payout", total_payout)
        .add_attribute("initial_payout", initial_payout)
        .add_attribute("unlocked_vesting", unlocked_vesting)
        .add_attribute("bonus", bonus))
}

pub fn execute_deposit_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_address: String,
    amount: Uint128,
    is_native: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if is_native {
        // must_pay enforces exactly one coin of the correct denom
        let paid = must_pay(&info, &token_address)?;
        if paid != amount {
            return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                format!("Expected {} but received {}", amount, paid),
            )));
        }
        
        Ok(Response::new()
            .add_attribute("action", "deposit_funds")
            .add_attribute("token_address", token_address)
            .add_attribute("amount", amount))
    } else {
        // CW20 token: use TransferFrom (requires allowance)
        let transfer_msg = WasmMsg::Execute {
            contract_addr: token_address.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount,
            })?,
            funds: vec![],
        };

        Ok(Response::new()
            .add_message(transfer_msg)
            .add_attribute("action", "deposit_funds")
            .add_attribute("token_address", token_address)
            .add_attribute("amount", amount))
    }
}

pub fn execute_withdraw_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_address: String,
    amount: Uint128,
    is_native: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    // Withdraw guard: refuse if any airdrop using this token is still
    // active OR has no expiry / its expiry has not yet passed. Forces the
    // admin to first `ToggleAirdrop(active=false)` and wait until
    // `expiry_time` before draining the pool — protects claimants while
    // an airdrop is live.
    let now = env.block.time.seconds();
    for kv in AIRDROPS.range(deps.storage, None, None, Order::Ascending) {
        let (id, airdrop) = kv?;
        if airdrop.token_address != token_address || airdrop.is_native != is_native {
            continue;
        }
        if airdrop.is_active {
            return Err(ContractError::WithdrawalBlocked { airdrop_id: id });
        }
        match airdrop.expiry_time {
            None => return Err(ContractError::WithdrawalBlockedNoExpiry { airdrop_id: id }),
            Some(exp) if now < exp => {
                return Err(ContractError::WithdrawalBlocked { airdrop_id: id });
            }
            Some(_) => { /* expired & inactive — does not block */ }
        }
    }

    let msg: CosmosMsg = if is_native {
        // Native token
        cosmwasm_std::BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![cosmwasm_std::Coin {
                denom: token_address.clone(),
                amount,
            }],
        }.into()
    } else {
        // CW20 token
        WasmMsg::Execute {
            contract_addr: token_address.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }.into()
    };

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "withdraw_funds")
        .add_attribute("token_address", token_address)
        .add_attribute("amount", amount))
}



pub fn execute_update_eligibility(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    airdrop_id: u64,
    eligibility: Vec<(String, Uint128)>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let airdrop = AIRDROPS
        .may_load(deps.storage, airdrop_id)?
        .ok_or(ContractError::AirdropNotFound {})?;

    if env.block.time.seconds() >= airdrop.start_time {
        return Err(ContractError::AirdropAlreadyStarted {});
    }

    if eligibility.len() > MAX_ELIGIBILITY_BATCH {
        return Err(ContractError::EligibilityBatchTooLarge {
            got: eligibility.len(),
            max: MAX_ELIGIBILITY_BATCH,
        });
    }

    // Maintain ELIGIBILITY_TOTAL correctly across overwrites:
    //   total += (new_amount - old_amount) for each entry.
    let mut total = ELIGIBILITY_TOTAL
        .may_load(deps.storage, airdrop_id)?
        .unwrap_or_default();

    for (addr, amount) in eligibility {
        let validated_addr = deps.api.addr_validate(&addr)?;
        let old_amount = ELIGIBILITY
            .may_load(deps.storage, (airdrop_id, &validated_addr))?
            .unwrap_or_default();
        ELIGIBILITY.save(deps.storage, (airdrop_id, &validated_addr), &amount)?;
        total = total
            .checked_sub(old_amount)
            .map_err(StdError::from)?
            .checked_add(amount)
            .map_err(StdError::from)?;
    }
    ELIGIBILITY_TOTAL.save(deps.storage, airdrop_id, &total)?;

    Ok(Response::new()
        .add_attribute("action", "update_eligibility")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("total_eligibility", total))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Airdrop { id } => to_json_binary(&query_airdrop(deps, id)?),
        QueryMsg::LatestAirdropId {} => to_json_binary(&query_latest_airdrop_id(deps)?),
        QueryMsg::UserAirdropStatus { id, address } => to_json_binary(&query_user_airdrop_status(deps, env, id, address)?),
        QueryMsg::Solvency { id } => to_json_binary(&query_solvency(deps, env, id)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        admin: config.admin,
    })
}

fn query_airdrop(deps: Deps, id: u64) -> StdResult<AirdropResponse> {
    let airdrop = AIRDROPS.load(deps.storage, id)?;
    Ok(AirdropResponse {
        id,
        token_address: airdrop.token_address,
        start_time: airdrop.start_time,
        vesting_start_time: airdrop.vesting_start_time,
        vesting_end_time: airdrop.vesting_end_time,
        initial_claim_percent: airdrop.initial_claim_percent,
        apy_bonus_bps: airdrop.apy_bonus_bps,
        is_active: airdrop.is_active,
        is_native: airdrop.is_native,
        expiry_time: airdrop.expiry_time,
    })
}


fn query_latest_airdrop_id(deps: Deps) -> StdResult<LatestAirdropIdResponse> {
    let id = LATEST_AIRDROP_ID.load(deps.storage)?;
    Ok(LatestAirdropIdResponse { id })
}

fn query_user_airdrop_status(deps: Deps, env: Env, id: u64, address: String) -> StdResult<UserAirdropStatusResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let airdrop = AIRDROPS.load(deps.storage, id)?;

    // Total allocation from eligibility map
    let total_allocation = ELIGIBILITY
        .may_load(deps.storage, (id, &addr))?
        .unwrap_or_default();

    // Check if user already claimed
    let claim = USER_CLAIMS.may_load(deps.storage, (id, &addr))?;

    if let Some(claim) = claim {
        // User has already claimed — return claimed state
        return Ok(UserAirdropStatusResponse {
            total_allocation,
            current_claimable: Uint128::zero(),
            is_claimed: true,
            amount_claimed: claim.amount_claimed,
            claim_time: Some(claim.claim_time),
            vesting_status: "claimed".to_string(),
        });
    }

    // Not yet claimed — compute current claimable based on vesting
    let current_time = env.block.time.seconds();

    if current_time < airdrop.start_time {
        // Airdrop hasn't started yet
        return Ok(UserAirdropStatusResponse {
            total_allocation,
            current_claimable: Uint128::zero(),
            is_claimed: false,
            amount_claimed: Uint128::zero(),
            claim_time: None,
            vesting_status: "not_started".to_string(),
        });
    }

    if let Some(expiry) = airdrop.expiry_time {
        if current_time >= expiry {
            return Ok(UserAirdropStatusResponse {
                total_allocation,
                current_claimable: Uint128::zero(),
                is_claimed: false,
                amount_claimed: Uint128::zero(),
                claim_time: None,
                vesting_status: "expired".to_string(),
            });
        }
    }

    // Initial portion (e.g. 40%)
    let initial_payout = total_allocation.multiply_ratio(airdrop.initial_claim_percent, 100u128);
    let vesting_base = total_allocation.saturating_sub(initial_payout);

    let mut unlocked_vesting = Uint128::zero();
    let mut bonus = Uint128::zero();
    let vesting_status;

    if current_time <= airdrop.vesting_start_time {
        // After start but before vesting begins — only initial portion available
        vesting_status = "not_started".to_string();
    } else if current_time < airdrop.vesting_end_time {
        // Mid-vesting
        let elapsed = current_time.saturating_sub(airdrop.vesting_start_time);
        let duration = airdrop.vesting_end_time.saturating_sub(airdrop.vesting_start_time);
        if duration > 0 {
            unlocked_vesting = vesting_base.multiply_ratio(elapsed, duration);
        }
        vesting_status = "in_progress".to_string();
    } else {
        // Vesting completed
        unlocked_vesting = vesting_base;
        bonus = vesting_base.multiply_ratio(airdrop.apy_bonus_bps, 10000u64);
        vesting_status = "completed".to_string();
    }

    let current_claimable = initial_payout
        .checked_add(unlocked_vesting).map_err(StdError::from)?
        .checked_add(bonus).map_err(StdError::from)?;

    Ok(UserAirdropStatusResponse {
        total_allocation,
        current_claimable,
        is_claimed: false,
        amount_claimed: Uint128::zero(),
        claim_time: None,
        vesting_status,
    })
}

fn query_solvency(deps: Deps, env: Env, id: u64) -> StdResult<SolvencyResponse> {
    let airdrop = AIRDROPS.load(deps.storage, id)?;

    let total_eligibility = ELIGIBILITY_TOTAL
        .may_load(deps.storage, id)?
        .unwrap_or_default();
    let total_claimed = CLAIMED_TOTAL.may_load(deps.storage, id)?.unwrap_or_default();
    let outstanding = total_eligibility.saturating_sub(total_claimed);

    // Worst case: every remaining recipient claims at/after vesting end
    // and collects the APY bonus. Bonus accrues on vesting_base = the
    // (100 - initial_claim_percent)% portion that's still locked up.
    //
    // `saturating_add`, not `checked_add`: at pathological inputs
    // (`total_eligibility ≈ Uint128::MAX`, `apy = 10_000 bps`,
    // `initial_claim_percent = 0`) `outstanding + max_bonus` overflows.
    // The query is a UI signal — a saturated `Uint128::MAX` with
    // `is_fully_funded = false` is strictly more useful than a hard error.
    let vesting_base = total_eligibility
        .multiply_ratio(100u128.saturating_sub(airdrop.initial_claim_percent as u128), 100u128);
    let max_bonus = vesting_base.multiply_ratio(airdrop.apy_bonus_bps, 10_000u64);
    let max_outstanding = outstanding.saturating_add(max_bonus);

    // Pull current contract balance for the airdrop's token.
    let contract_balance: Uint128 = if airdrop.is_native {
        deps.querier
            .query_balance(&env.contract.address, &airdrop.token_address)?
            .amount
    } else {
        let resp: cw20::BalanceResponse = deps.querier.query_wasm_smart(
            &airdrop.token_address,
            &cw20::Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        resp.balance
    };

    Ok(SolvencyResponse {
        airdrop_id: id,
        token_address: airdrop.token_address,
        is_native: airdrop.is_native,
        total_eligibility,
        total_claimed,
        outstanding,
        max_outstanding,
        contract_balance,
        is_fully_funded: contract_balance >= max_outstanding,
    })
}
