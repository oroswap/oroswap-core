use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, WasmMsg, StdError,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    AirdropResponse, ClaimChoice, UserClaimInfo, UserClaimResponse, ConfigResponse, EligibilityResponse, ExecuteMsg,
    InstantiateMsg, LatestAirdropIdResponse, QueryMsg,
};
use crate::state::{Config, AIRDROPS, USER_CLAIMS, CONFIG, ELIGIBILITY, LATEST_AIRDROP_ID, AirdropInfo, UserClaim};

const CONTRACT_NAME: &str = "oroswap-airdrop";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
            start_time,
            end_time,
            vesting_duration,
            initial_claim_percent,
            apy_bonus_bps,
            eligibility,
        } => execute_add_airdrop(
            deps,
            info,
            token_address,
            start_time,
            end_time,
            vesting_duration,
            initial_claim_percent,
            apy_bonus_bps,
            eligibility,
        ),
        ExecuteMsg::Claim { airdrop_id, choice } => execute_claim(deps, env, info, airdrop_id, choice),
        ExecuteMsg::WithdrawVested { airdrop_id } => execute_withdraw_vested(deps, env, info, airdrop_id),
        ExecuteMsg::DepositFunds { token_address, amount } => execute_deposit_funds(deps, env, info, token_address, amount),
        ExecuteMsg::WithdrawFunds { token_address, amount } => execute_withdraw_funds(deps, info, token_address, amount),
        ExecuteMsg::UpdateConfig { admin } => {
            execute_update_config(deps, info, admin)
        }
    }
}

pub fn execute_add_airdrop(
    deps: DepsMut,
    info: MessageInfo,
    token_address: String,
    start_time: u64,
    end_time: u64,
    vesting_duration: u64,
    initial_claim_percent: u64,
    apy_bonus_bps: u64,
    eligibility: Vec<(String, Uint128)>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if start_time >= end_time {
        return Err(ContractError::InvalidTime {});
    }

    if initial_claim_percent > 100 {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "initial_claim_percent cannot exceed 100",
        )));
    }

    let airdrop_id = LATEST_AIRDROP_ID.load(deps.storage)? + 1;
    LATEST_AIRDROP_ID.save(deps.storage, &airdrop_id)?;

    AIRDROPS.save(
        deps.storage,
        airdrop_id,
        &AirdropInfo {
            token_address: token_address.clone(),
            start_time,
            end_time,
            vesting_duration,
            initial_claim_percent,
            apy_bonus_bps,
        },
    )?;

    for (addr, amount) in eligibility {
        let validated_addr = deps.api.addr_validate(&addr)?;
        ELIGIBILITY.save(deps.storage, (airdrop_id, &validated_addr), &amount)?;
    }

    Ok(Response::new()
        .add_attribute("action", "add_airdrop")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("token_address", token_address)
        .add_attribute("start_time", start_time.to_string())
        .add_attribute("end_time", end_time.to_string()))
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    airdrop_id: u64,
    choice: ClaimChoice,
) -> Result<Response, ContractError> {
    let airdrop = AIRDROPS
        .may_load(deps.storage, airdrop_id)?
        .ok_or(ContractError::AirdropNotFound {})?;

    let current_time = env.block.time.seconds();

    if current_time < airdrop.start_time {
        return Err(ContractError::AirdropNotStarted {});
    }

    if current_time > airdrop.end_time {
        return Err(ContractError::AirdropExpired {});
    }

    if USER_CLAIMS.has(deps.storage, (airdrop_id, &info.sender)) {
        return Err(ContractError::AlreadyClaimed {});
    }

    let amount_eligible = ELIGIBILITY
        .may_load(deps.storage, (airdrop_id, &info.sender))?
        .ok_or(ContractError::NotEligible {})?;

    let mut messages: Vec<CosmosMsg> = vec![];
    let (initial_claimed, vesting_amount) = if choice == ClaimChoice::InitialPortion {
        let initial = amount_eligible.multiply_ratio(airdrop.initial_claim_percent, 100u128);
        if !initial.is_zero() {
            let msg: CosmosMsg = if airdrop.token_address.contains('.') || airdrop.token_address.len() < 20 {
                cosmwasm_std::BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: vec![cosmwasm_std::Coin {
                        denom: airdrop.token_address.clone(),
                        amount: initial,
                    }],
                }.into()
            } else {
                WasmMsg::Execute {
                    contract_addr: airdrop.token_address.clone(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: info.sender.to_string(),
                        amount: initial,
                    })?,
                    funds: vec![],
                }.into()
            };
            messages.push(msg);
        }
        (initial, Uint128::zero())
    } else {
        (Uint128::zero(), amount_eligible)
    };

    let user_claim = UserClaim {
        amount_eligible,
        initial_claimed,
        vesting_amount,
        start_time: current_time,
        last_withdrawal_time: current_time,
        withdrawn_amount: Uint128::zero(),
    };

    USER_CLAIMS.save(deps.storage, (airdrop_id, &info.sender), &user_claim)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("user", info.sender)
        .add_attribute("amount_eligible", amount_eligible)
        .add_attribute("initial_claimed", initial_claimed)
        .add_attribute("vesting_amount", vesting_amount))
}

pub fn execute_withdraw_vested(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    airdrop_id: u64,
) -> Result<Response, ContractError> {
    let airdrop = AIRDROPS
        .may_load(deps.storage, airdrop_id)?
        .ok_or(ContractError::AirdropNotFound {})?;

    let mut user_claim = USER_CLAIMS
        .may_load(deps.storage, (airdrop_id, &info.sender))?
        .ok_or(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "No active claim found for user",
        )))?;

    let current_time = env.block.time.seconds();
    
    // 1. Calculate Unlocked Amount (Linear Vesting)
    let elapsed = current_time.saturating_sub(user_claim.start_time);
    let unlocked_total = if elapsed >= airdrop.vesting_duration {
        user_claim.vesting_amount
    } else {
        user_claim.vesting_amount.multiply_ratio(elapsed, airdrop.vesting_duration)
    };
    
    let claimable_unlocked = unlocked_total.saturating_sub(user_claim.withdrawn_amount);

    // 2. Calculate APY Bonus
    const SECONDS_PER_YEAR: u64 = 365 * 24 * 3600;
    
    let time_held = current_time.saturating_sub(user_claim.last_withdrawal_time);
    
    let remaining_principal = user_claim.vesting_amount.saturating_sub(user_claim.withdrawn_amount);
    
    let apy_bonus = if !remaining_principal.is_zero() && airdrop.apy_bonus_bps > 0 && time_held > 0 {
        remaining_principal
            .multiply_ratio(airdrop.apy_bonus_bps, 10000u128)
            .multiply_ratio(time_held, SECONDS_PER_YEAR)
    } else {
        Uint128::zero()
    };

    let total_payout = claimable_unlocked.checked_add(apy_bonus).map_err(StdError::from)?;

    if total_payout.is_zero() {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "Nothing to withdraw at this time",
        )));
    }

    // Update State
    user_claim.withdrawn_amount = user_claim.withdrawn_amount.checked_add(claimable_unlocked).map_err(StdError::from)?;
    user_claim.last_withdrawal_time = current_time;
    USER_CLAIMS.save(deps.storage, (airdrop_id, &info.sender), &user_claim)?;

    let msg: CosmosMsg = if airdrop.token_address.contains('.') || airdrop.token_address.len() < 20 {
        cosmwasm_std::BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![cosmwasm_std::Coin {
                denom: airdrop.token_address.clone(),
                amount: total_payout,
            }],
        }.into()
    } else {
        WasmMsg::Execute {
            contract_addr: airdrop.token_address.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: total_payout,
            })?,
            funds: vec![],
        }.into()
    };

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "withdraw_vested")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("user", info.sender)
        .add_attribute("payout", total_payout)
        .add_attribute("apy_bonus", apy_bonus))
}

pub fn execute_deposit_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_address: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if token_address.contains('.') || token_address.len() < 20 {
        // Native token: verify that the funds were sent with the message
        let coin = info
            .funds
            .iter()
            .find(|c| c.denom == token_address && c.amount == amount)
            .ok_or(ContractError::Std(cosmwasm_std::StdError::generic_err(
                "Correct native funds not sent",
            )))?;
        
        Ok(Response::new()
            .add_attribute("action", "deposit_funds")
            .add_attribute("token_address", token_address)
            .add_attribute("amount", coin.amount))
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
    info: MessageInfo,
    token_address: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let msg: CosmosMsg = if token_address.contains('.') || token_address.len() < 20 {
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

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    admin: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(admin) = admin {
        config.admin = deps.api.addr_validate(&admin)?;
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Airdrop { id } => to_json_binary(&query_airdrop(deps, id)?),
        QueryMsg::Eligibility { id, address } => to_json_binary(&query_eligibility(deps, id, address)?),
        QueryMsg::ClaimStatus { id, address } => to_json_binary(&query_claim_status(deps, id, address)?),
        QueryMsg::LatestAirdropId {} => to_json_binary(&query_latest_airdrop_id(deps)?),
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
        end_time: airdrop.end_time,
        vesting_duration: airdrop.vesting_duration,
        initial_claim_percent: airdrop.initial_claim_percent,
        apy_bonus_bps: airdrop.apy_bonus_bps,
    })
}

fn query_eligibility(deps: Deps, id: u64, address: String) -> StdResult<EligibilityResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let amount = ELIGIBILITY
        .may_load(deps.storage, (id, &addr))?
        .unwrap_or_default();
    Ok(EligibilityResponse { amount })
}

fn query_claim_status(deps: Deps, id: u64, address: String) -> StdResult<UserClaimResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let claim = USER_CLAIMS.may_load(deps.storage, (id, &addr))?.map(|c| UserClaimInfo {
        amount_eligible: c.amount_eligible,
        initial_claimed: c.initial_claimed,
        vesting_amount: c.vesting_amount,
        start_time: c.start_time,
        last_withdrawal_time: c.last_withdrawal_time,
        withdrawn_amount: c.withdrawn_amount,
    });
    Ok(UserClaimResponse { claim })
}

fn query_latest_airdrop_id(deps: Deps) -> StdResult<LatestAirdropIdResponse> {
    let id = LATEST_AIRDROP_ID.load(deps.storage)?;
    Ok(LatestAirdropIdResponse { id })
}
