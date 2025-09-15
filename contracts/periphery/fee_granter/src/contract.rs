use cosmos_sdk_proto::cosmos::base::v1beta1::Coin as SdkCoin;
use cosmos_sdk_proto::cosmos::feegrant::v1beta1::{
    BasicAllowance, MsgGrantAllowance, MsgRevokeAllowance,
};
use cosmos_sdk_proto::prost::Message;
use cosmos_sdk_proto::traits::TypeUrl;
use cosmos_sdk_proto::Any;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coins, Addr, BankMsg, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_utils::must_pay;

use oroswap::asset::validate_native_denom;
use oroswap::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};
use oroswap::fee_granter::{Config, ExecuteMsg, InstantiateMsg};

use crate::error::ContractError;
use crate::state::{update_admins_with_validation, CONFIG, GRANTS, OWNERSHIP_PROPOSAL};

pub(crate) const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, StdError> {
    let attrs = vec![
        attr("action", "instantiate"),
        attr("contract", CONTRACT_NAME),
        attr("gas_denom", &msg.gas_denom),
    ];

    validate_native_denom(&msg.gas_denom)?;
    CONFIG.save(
        deps.storage,
        &Config {
            owner: deps.api.addr_validate(&msg.owner)?,
            admins: update_admins_with_validation(deps.api, vec![], &msg.admins, &[])?,
            gas_denom: msg.gas_denom,
        },
    )?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default().add_attributes(attrs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Grant {
            grantee_contract,
            amount,
            bypass_amount_check,
        } => {
            let grantee_contract = deps.api.addr_validate(&grantee_contract)?;
            grant(
                deps,
                env,
                info,
                grantee_contract,
                amount,
                bypass_amount_check,
            )
        }
        ExecuteMsg::Revoke { grantee_contract } => {
            let grantee_contract = deps.api.addr_validate(&grantee_contract)?;
            revoke(deps, env, info, grantee_contract)
        }
        ExecuteMsg::TransferCoins { amount, receiver } => {
            transfer_coins(deps, info, amount, receiver)
        }
        ExecuteMsg::UpdateAdmins { add, remove } => update_admins(deps, info, add, remove),
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
    }
}

fn grant(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    grantee_contract: Addr,
    amount: Uint128,
    bypass_amount_check: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender && !config.admins.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    if !bypass_amount_check {
        let sent_amount = must_pay(&info, &config.gas_denom)?;
        if sent_amount != amount {
            return Err(ContractError::InvalidAmount {
                expected: amount,
                actual: sent_amount,
            });
        }
    }

    // Always verify that the contract has sufficient balance to grant the requested amount
    // This prevents creating grants that cannot be realistically spent
    let contract_balance = deps
        .querier
        .query_balance(&env.contract.address, &config.gas_denom)?
        .amount;
    
    if contract_balance < amount {
        return Err(ContractError::InsufficientBalance {
            requested: amount,
            available: contract_balance,
        });
    }

    GRANTS.update(
        deps.storage,
        &grantee_contract,
        |existing| -> StdResult<_> {
            match existing {
                None => Ok(amount),
                Some(_) => Err(StdError::generic_err(format!(
                    "Grant already exists for {grantee_contract}",
                ))),
            }
        },
    )?;

    let allowance = BasicAllowance {
        spend_limit: vec![SdkCoin {
            denom: config.gas_denom,
            amount: amount.to_string(),
        }],
        expiration: None,
    };
    let grant_msg = MsgGrantAllowance {
        granter: env.contract.address.to_string(),
        grantee: grantee_contract.to_string(),
        allowance: Some(Any {
            type_url: BasicAllowance::TYPE_URL.to_string(),
            value: allowance.encode_to_vec(),
        }),
    };

    let msg = CosmosMsg::Stargate {
        type_url: MsgGrantAllowance::TYPE_URL.to_string(),
        value: grant_msg.encode_to_vec().into(),
    };
    Ok(Response::default().add_message(msg).add_attributes([
        ("action", "grant"),
        ("grantee_contract", grantee_contract.as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

fn revoke(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    grantee_contract: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender && !config.admins.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    GRANTS.remove(deps.storage, &grantee_contract);

    let revoke_msg = MsgRevokeAllowance {
        granter: env.contract.address.to_string(),
        grantee: grantee_contract.to_string(),
    };
    let msg = CosmosMsg::Stargate {
        type_url: MsgRevokeAllowance::TYPE_URL.to_string(),
        value: revoke_msg.encode_to_vec().into(),
    };

    Ok(Response::default().add_message(msg).add_attributes([
        ("action", "revoke"),
        ("grantee_contract", grantee_contract.as_str()),
    ]))
}

fn transfer_coins(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
    receiver: Option<String>,
) -> Result<Response, ContractError> {
    if amount.is_zero() {
        return Err(StdError::generic_err("Can't send 0 amount").into());
    }
    let config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender && !config.admins.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: receiver.unwrap_or(info.sender.to_string()),
        amount: coins(amount.u128(), config.gas_denom),
    });
    Ok(Response::default().add_message(send_msg).add_attributes([
        ("action", "transfer_coins"),
        ("amount", amount.to_string().as_str()),
    ]))
}

fn update_admins(
    deps: DepsMut,
    info: MessageInfo,
    add_admins: Vec<String>,
    remove_admins: Vec<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    config.admins =
        update_admins_with_validation(deps.api, config.admins, &add_admins, &remove_admins)?;
    CONFIG.save(deps.storage, &config)?;

    let mut attributes = vec![attr("action", "update_admins")];
    if !add_admins.is_empty() {
        attributes.push(attr("add_admins", add_admins.join(",")));
    }
    if !remove_admins.is_empty() {
        attributes.push(attr("remove_admins", remove_admins.join(",")));
    }

    Ok(Response::default().add_attributes(attributes))
}
