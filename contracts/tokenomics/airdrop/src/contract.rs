use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, WasmMsg, StdError,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    AirdropResponse, UserClaimInfo, UserClaimResponse, ConfigResponse, EligibilityResponse, ExecuteMsg,
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
            vesting_start_time,
            vesting_end_time,
            initial_claim_percent,
            apy_bonus_bps,
            eligibility,
        } => execute_add_airdrop(
            deps,
            info,
            token_address,
            start_time,
            vesting_start_time,
            vesting_end_time,
            initial_claim_percent,
            apy_bonus_bps,
            eligibility,
        ),
        ExecuteMsg::ToggleAirdrop { id, active } => {
            execute_toggle_airdrop(deps, info, id, active)
        }
        ExecuteMsg::Claim { airdrop_id } => execute_claim(deps, env, info, airdrop_id),
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
    vesting_start_time: u64,
    vesting_end_time: u64,
    initial_claim_percent: u64,
    apy_bonus_bps: u64,
    eligibility: Vec<(String, Uint128)>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if vesting_start_time < start_time {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "vesting_start_time must be >= start_time",
        )));
    }
    if vesting_start_time >= vesting_end_time {
        return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
            "vesting_start_time must be before vesting_end_time",
        )));
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
            id: airdrop_id,
            token_address: token_address.clone(),
            start_time,
            vesting_start_time,
            vesting_end_time,
            initial_claim_percent,
            apy_bonus_bps,
            is_active: true,
        },
    )?;

    for (addr, amount) in eligibility {
        let validated_addr = deps.api.addr_validate(&addr)?;
        ELIGIBILITY.save(deps.storage, (airdrop_id, &validated_addr), &amount)?;
    }

    Ok(Response::new()
        .add_attribute("action", "add_airdrop")
        .add_attribute("airdrop_id", airdrop_id.to_string())
        .add_attribute("token_address", token_address))
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

    // Single claim check
    if USER_CLAIMS.has(deps.storage, (airdrop_id, &info.sender)) {
        return Err(ContractError::AlreadyClaimed {});
    }

    let amount_eligible = ELIGIBILITY
        .may_load(deps.storage, (airdrop_id, &info.sender))?
        .ok_or(ContractError::NotEligible {})?;

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
                bonus = Uint128::from(vesting_base.u128() * airdrop.apy_bonus_bps as u128 / 10000);
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

    let mut messages: Vec<CosmosMsg> = vec![];
    if !total_payout.is_zero() {
        let msg: CosmosMsg = if airdrop.token_address.contains('.') || airdrop.token_address.len() < 20 {
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
        vesting_start_time: airdrop.vesting_start_time,
        vesting_end_time: airdrop.vesting_end_time,
        initial_claim_percent: airdrop.initial_claim_percent,
        apy_bonus_bps: airdrop.apy_bonus_bps,
        is_active: airdrop.is_active,
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
        amount_claimed: c.amount_claimed,
        claim_time: c.claim_time,
    });
    Ok(UserClaimResponse { claim })
}

fn query_latest_airdrop_id(deps: Deps) -> StdResult<LatestAirdropIdResponse> {
    let id = LATEST_AIRDROP_ID.load(deps.storage)?;
    Ok(LatestAirdropIdResponse { id })
}
