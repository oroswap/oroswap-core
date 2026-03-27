use cosmwasm_std::{
    attr, coin, entry_point, to_json_binary, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    BalanceResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, KeepersResponse, MigrateMsg,
    QueryMsg,
};
use crate::state::{Config, CONFIG, DEFAULT_IBC_TIMEOUT_SECS};

const CONTRACT_NAME: &str = "oroswap-revenue-router";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── Instantiate ───────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        usdc_denom: msg.usdc_denom.clone(),
        dest_chain_id: msg.dest_chain_id.clone(),
        dest_usdc_denom: msg.dest_usdc_denom.clone(),
        dest_receiver: msg.dest_receiver.clone(),
        ibc_channel: msg.ibc_channel.clone(),
        ibc_timeout_secs: msg.ibc_timeout_secs.unwrap_or(DEFAULT_IBC_TIMEOUT_SECS),
        keepers: vec![],
    };

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attributes([
        attr("action", "instantiate"),
        attr("owner", &msg.owner),
        attr("usdc_denom", &msg.usdc_denom),
        attr("dest_chain_id", &msg.dest_chain_id),
        attr("ibc_channel", &msg.ibc_channel),
    ]))
}

// ── Execute ───────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BridgeUsdc { amount, memo, receiver } => {
            bridge_usdc(deps, env, info, amount, memo, receiver)
        }
        ExecuteMsg::UpdateConfig {
            owner,
            usdc_denom,
            dest_chain_id,
            dest_usdc_denom,
            dest_receiver,
            ibc_channel,
            ibc_timeout_secs,
        } => update_config(
            deps, info, owner, usdc_denom, dest_chain_id, dest_usdc_denom,
            dest_receiver, ibc_channel, ibc_timeout_secs,
        ),
        ExecuteMsg::AddKeeper { keeper } => add_keeper(deps, info, keeper),
        ExecuteMsg::RemoveKeeper { keeper } => remove_keeper(deps, info, keeper),
        ExecuteMsg::EmergencyWithdraw { denom, amount, recipient } => {
            emergency_withdraw(deps, env, info, denom, amount, recipient)
        }
    }
}

/// Bridge USDC to Ethereum via a single ICS-20 MsgTransfer with a Skip PFM memo.
///
/// The keeper is responsible for calling Skip API off-chain:
///   1. POST /v2/fungible/route  — get the multi-hop route
///   2. POST /v2/fungible/msgs   — get the MsgTransfer memo + receiver
/// Then call this with the resulting `memo` and `receiver`.
fn bridge_usdc(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
    memo: String,
    receiver: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    only_keeper_or_owner(&cfg, &info)?;

    let balance = usdc_balance(&deps.as_ref(), &env, &cfg.usdc_denom)?;
    if balance.is_zero() {
        return Err(ContractError::ZeroBalance {});
    }

    let bridge_amount = match amount {
        Some(a) => {
            if a > balance {
                return Err(ContractError::InsufficientBalance {
                    requested: a.to_string(),
                    balance: balance.to_string(),
                });
            }
            a
        }
        None => balance,
    };

    // cosmwasm-std 1.5 IbcMsg::Transfer has no memo field, so we use
    // CosmosMsg::Stargate to encode ibc.applications.transfer.v1.MsgTransfer
    // directly — this is the only way to pass the Skip PFM memo.
    let timeout_nanos = env.block.time.nanos() + cfg.ibc_timeout_secs * 1_000_000_000;
    let msg = stargate_ibc_transfer(
        &env.contract.address.to_string(),
        &cfg.ibc_channel,
        &receiver,
        bridge_amount.u128(),
        &cfg.usdc_denom,
        timeout_nanos,
        &memo,
    );

    Ok(Response::new()
        .add_message(msg)
        .add_attributes([
            attr("action", "bridge_usdc"),
            attr("amount", bridge_amount),
            attr("channel", &cfg.ibc_channel),
            attr("receiver", &receiver),
            attr("dest_chain_id", &cfg.dest_chain_id),
        ]))
}

#[allow(clippy::too_many_arguments)]
fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    usdc_denom: Option<String>,
    dest_chain_id: Option<String>,
    dest_usdc_denom: Option<String>,
    dest_receiver: Option<String>,
    ibc_channel: Option<String>,
    ibc_timeout_secs: Option<u64>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(v) = owner        { cfg.owner = deps.api.addr_validate(&v)?; }
    if let Some(v) = usdc_denom   { cfg.usdc_denom = v; }
    if let Some(v) = dest_chain_id { cfg.dest_chain_id = v; }
    if let Some(v) = dest_usdc_denom { cfg.dest_usdc_denom = v; }
    if let Some(v) = dest_receiver { cfg.dest_receiver = v; }
    if let Some(v) = ibc_channel  { cfg.ibc_channel = v; }
    if let Some(v) = ibc_timeout_secs { cfg.ibc_timeout_secs = v; }
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

fn add_keeper(deps: DepsMut, info: MessageInfo, keeper: String) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner { return Err(ContractError::Unauthorized {}); }
    let addr = deps.api.addr_validate(&keeper)?;
    if !cfg.keepers.contains(&addr) { cfg.keepers.push(addr); }
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", "add_keeper")
        .add_attribute("keeper", keeper))
}

fn remove_keeper(deps: DepsMut, info: MessageInfo, keeper: String) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner { return Err(ContractError::Unauthorized {}); }
    let addr = deps.api.addr_validate(&keeper)?;
    cfg.keepers.retain(|k| k != &addr);
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", "remove_keeper")
        .add_attribute("keeper", keeper))
}

fn emergency_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
    amount: Option<Uint128>,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner { return Err(ContractError::Unauthorized {}); }

    let withdraw_amount = match amount {
        Some(a) => a,
        None => deps.querier.query_balance(&env.contract.address, &denom)?.amount,
    };
    if withdraw_amount.is_zero() { return Err(ContractError::ZeroBalance {}); }

    let to = recipient.unwrap_or_else(|| cfg.owner.to_string());
    Ok(Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: to.clone(),
            amount: vec![coin(withdraw_amount.u128(), &denom)],
        }))
        .add_attributes([
            attr("action", "emergency_withdraw"),
            attr("denom", &denom),
            attr("amount", withdraw_amount),
            attr("recipient", &to),
        ]))
}

// ── Query ─────────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::UsdcBalance {} => to_json_binary(&query_usdc_balance(deps, env)?),
        QueryMsg::Keepers {} => to_json_binary(&query_keepers(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        usdc_denom: cfg.usdc_denom,
        dest_chain_id: cfg.dest_chain_id,
        dest_usdc_denom: cfg.dest_usdc_denom,
        dest_receiver: cfg.dest_receiver,
        ibc_channel: cfg.ibc_channel,
        ibc_timeout_secs: cfg.ibc_timeout_secs,
    })
}

fn query_usdc_balance(deps: Deps, env: Env) -> StdResult<BalanceResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let amount = usdc_balance(&deps, &env, &cfg.usdc_denom)?;
    Ok(BalanceResponse { denom: cfg.usdc_denom, amount })
}

fn query_keepers(deps: Deps) -> StdResult<KeepersResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(KeepersResponse { keepers: cfg.keepers.iter().map(|a| a.to_string()).collect() })
}

// ── Migrate ───────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn only_keeper_or_owner(cfg: &Config, info: &MessageInfo) -> Result<(), ContractError> {
    if info.sender == cfg.owner || cfg.keepers.contains(&info.sender) {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}

fn usdc_balance(deps: &Deps, env: &Env, denom: &str) -> StdResult<Uint128> {
    Ok(deps.querier.query_balance(&env.contract.address, denom)?.amount)
}

/// Build a CosmosMsg::Stargate for ibc.applications.transfer.v1.MsgTransfer.
///
/// cosmwasm-std 1.5 IbcMsg::Transfer has no memo field, so we hand-encode
/// the protobuf directly. The memo carries Skip's PFM forwarding instructions.
pub(crate) fn stargate_ibc_transfer(
    sender: &str,
    channel_id: &str,
    receiver: &str,
    amount: u128,
    denom: &str,
    timeout_timestamp_nanos: u64,
    memo: &str,
) -> CosmosMsg {
    let mut buf = Vec::new();
    // field 1: source_port = "transfer"
    pb_string(&mut buf, 1, "transfer");
    // field 2: source_channel
    pb_string(&mut buf, 2, channel_id);
    // field 3: token (Coin message)
    let mut coin_buf = Vec::new();
    pb_string(&mut coin_buf, 1, denom);
    pb_string(&mut coin_buf, 2, &amount.to_string());
    pb_bytes(&mut buf, 3, &coin_buf);
    // field 4: sender
    pb_string(&mut buf, 4, sender);
    // field 5: receiver
    pb_string(&mut buf, 5, receiver);
    // field 6: timeout_height — omitted (timestamp-only timeout)
    // field 7: timeout_timestamp
    pb_uint64(&mut buf, 7, timeout_timestamp_nanos);
    // field 8: memo
    if !memo.is_empty() {
        pb_string(&mut buf, 8, memo);
    }

    CosmosMsg::Stargate {
        type_url: "/ibc.applications.transfer.v1.MsgTransfer".to_string(),
        value: Binary::from(buf),
    }
}

// ── Minimal protobuf encoding ─────────────────────────────────────────────────

fn pb_varint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 { buf.push(b); break; } else { buf.push(b | 0x80); }
    }
}
fn pb_tag(buf: &mut Vec<u8>, field: u32, wire: u32) {
    pb_varint(buf, ((field as u64) << 3) | wire as u64);
}
fn pb_string(buf: &mut Vec<u8>, field: u32, v: &str) {
    pb_bytes(buf, field, v.as_bytes());
}
fn pb_bytes(buf: &mut Vec<u8>, field: u32, v: &[u8]) {
    pb_tag(buf, field, 2);
    pb_varint(buf, v.len() as u64);
    buf.extend_from_slice(v);
}
fn pb_uint64(buf: &mut Vec<u8>, field: u32, v: u64) {
    if v != 0 { pb_tag(buf, field, 0); pb_varint(buf, v); }
}
