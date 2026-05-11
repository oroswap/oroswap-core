use cosmwasm_std::{
    attr, coin, entry_point, to_json_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, Reply, Response, StdError, StdResult, SubMsg, SubMsgResult, Uint128,
};
pub const BRIDGE_REPLY_ID: u64 = 1;
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use oroswap::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};

use crate::error::ContractError;
use crate::msg::{
    BalanceResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, KeepersResponse, MigrateMsg,
    PreviewBridgeResponse, QueryMsg,
};
use crate::state::{
    BridgeAttempt, Config, CONFIG, DEFAULT_IBC_TIMEOUT_SECS, KEEPERS, LAST_BRIDGE_ATTEMPT,
    MAX_IBC_TIMEOUT_SECS, OWNERSHIP_PROPOSAL,
};

/// Default page size for `Keepers` paginated query.
const DEFAULT_KEEPER_LIMIT: u32 = 30;
/// Maximum entries a single `Keepers` page can return.
const MAX_KEEPER_LIMIT: u32 = 100;

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

    let ibc_timeout_secs = msg.ibc_timeout_secs.unwrap_or(DEFAULT_IBC_TIMEOUT_SECS);
    if ibc_timeout_secs > MAX_IBC_TIMEOUT_SECS {
        return Err(ContractError::InvalidTimeout {});
    }

    validate_first_hop_receiver(&msg.first_hop_receiver)?;
    validate_forward_memo(&msg.forward_memo)?;
    if let Some(cap) = msg.max_bridge_per_tx {
        validate_bridge_cap(cap)?;
    }

    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        usdc_denom: msg.usdc_denom.clone(),
        ibc_channel: msg.ibc_channel.clone(),
        ibc_timeout_secs,
        first_hop_receiver: msg.first_hop_receiver.clone(),
        forward_memo: msg.forward_memo.clone(),
        max_bridge_per_tx: msg.max_bridge_per_tx,
        paused: false,
    };

    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new().add_attributes([
        attr("action", "instantiate"),
        attr("owner", &msg.owner),
        attr("usdc_denom", &msg.usdc_denom),
        attr("ibc_channel", &msg.ibc_channel),
        attr("first_hop_receiver", &msg.first_hop_receiver),
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
        ExecuteMsg::BridgeUsdc { amount } => bridge_usdc(deps, env, info, amount),
        ExecuteMsg::UpdateConfig {
            usdc_denom,
            ibc_channel,
            ibc_timeout_secs,
            first_hop_receiver,
            forward_memo,
            max_bridge_per_tx,
        } => update_config(
            deps,
            env,
            info,
            usdc_denom,
            ibc_channel,
            ibc_timeout_secs,
            first_hop_receiver,
            forward_memo,
            max_bridge_per_tx,
        ),
        ExecuteMsg::SetPaused { paused } => set_paused(deps, info, paused),
        ExecuteMsg::AddKeeper { keeper } => add_keeper(deps, info, keeper),
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
        ExecuteMsg::RemoveKeeper { keeper } => remove_keeper(deps, info, keeper),
        ExecuteMsg::EmergencyWithdraw { denom, amount, recipient } => {
            emergency_withdraw(deps, env, info, denom, amount, recipient)
        }
    }
}

/// Bridge USDC out of this contract to the destination encoded in Config.
///
/// The caller (owner or any registered keeper) can only choose how much to
/// bridge. The first-hop receiver and the forwarding memo come exclusively
/// from `Config`, which is owner-controlled. A hijacked keeper therefore
/// cannot redirect funds — at worst they can trigger a transfer to the
/// already-configured treasury.
fn bridge_usdc(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.paused {
        return Err(ContractError::Paused {});
    }
    only_keeper_or_owner(&cfg, deps.as_ref(), &info)?;

    let balance = usdc_balance(&deps.as_ref(), &env, &cfg.usdc_denom)?;
    if balance.is_zero() {
        return Err(ContractError::ZeroBalance {});
    }

    let bridge_amount = match amount {
        Some(a) => {
            if a.is_zero() {
                return Err(ContractError::ZeroBalance {});
            }
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

    // Bound a hijacked-keeper's per-call blast radius. The keeper still
    // can't redirect funds (receiver + memo are from Config), but the cap
    // limits how much can move per tx — useful when paired with monitoring
    // / a pause-authority response.
    if let Some(cap) = cfg.max_bridge_per_tx {
        if bridge_amount > cap {
            return Err(ContractError::BridgeCapExceeded {
                requested: bridge_amount.to_string(),
                cap: cap.to_string(),
            });
        }
    }

    // cosmwasm-std 1.5 IbcMsg::Transfer has no memo field, so we hand-encode
    // ibc.applications.transfer.v1.MsgTransfer via Stargate. Both `receiver`
    // and `memo` come from Config (owner-set), never from the caller.
    let timeout_nanos = cfg
        .ibc_timeout_secs
        .checked_mul(1_000_000_000)
        .and_then(|ns| env.block.time.nanos().checked_add(ns))
        .ok_or_else(|| ContractError::Std(StdError::generic_err("timeout overflow")))?;

    let msg = stargate_ibc_transfer(
        &env.contract.address.to_string(),
        &cfg.ibc_channel,
        &cfg.first_hop_receiver,
        bridge_amount.u128(),
        &cfg.usdc_denom,
        timeout_nanos,
        &cfg.forward_memo,
    );

    // Stash the context so the reply handler can echo it back as
    // attributes — useful for indexers and incident response. The Item
    // is always cleared by the reply handler within the same tx (we use
    // `reply_always`), so it never persists across blocks.
    LAST_BRIDGE_ATTEMPT.save(
        deps.storage,
        &BridgeAttempt {
            amount: bridge_amount,
            receiver: cfg.first_hop_receiver.clone(),
            channel: cfg.ibc_channel.clone(),
            denom: cfg.usdc_denom.clone(),
        },
    )?;

    let sub_msg = SubMsg::reply_always(msg, BRIDGE_REPLY_ID);

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_attributes([
            attr("action", "bridge_usdc"),
            attr("keeper", info.sender.to_string()),
            attr("amount", bridge_amount),
            attr("channel", &cfg.ibc_channel),
            attr("first_hop_receiver", &cfg.first_hop_receiver),
        ]))
}

#[allow(clippy::too_many_arguments)]
fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    usdc_denom: Option<String>,
    ibc_channel: Option<String>,
    ibc_timeout_secs: Option<u64>,
    first_hop_receiver: Option<String>,
    forward_memo: Option<String>,
    max_bridge_per_tx: Option<Option<Uint128>>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(v) = usdc_denom {
        let old_balance = usdc_balance(&deps.as_ref(), &env, &cfg.usdc_denom)?;
        if !old_balance.is_zero() {
            return Err(ContractError::StrandedBalance {});
        }
        cfg.usdc_denom = v;
    }
    if let Some(v) = ibc_channel {
        cfg.ibc_channel = v;
    }
    if let Some(v) = ibc_timeout_secs {
        if v > MAX_IBC_TIMEOUT_SECS {
            return Err(ContractError::InvalidTimeout {});
        }
        cfg.ibc_timeout_secs = v;
    }
    if let Some(v) = first_hop_receiver {
        validate_first_hop_receiver(&v)?;
        cfg.first_hop_receiver = v;
    }
    if let Some(v) = forward_memo {
        validate_forward_memo(&v)?;
        cfg.forward_memo = v;
    }
    if let Some(v) = max_bridge_per_tx {
        // Outer Some = "this field was supplied". Inner Option lets the
        // owner explicitly clear the cap (Some(None)) or set a new value
        // (Some(Some(x))). Validate when setting.
        if let Some(cap) = v {
            validate_bridge_cap(cap)?;
        }
        cfg.max_bridge_per_tx = v;
    }
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

fn set_paused(deps: DepsMut, info: MessageInfo, paused: bool) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }
    cfg.paused = paused;
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", "set_paused")
        .add_attribute("paused", paused.to_string()))
}

fn add_keeper(deps: DepsMut, info: MessageInfo, keeper: String) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }
    let addr = deps.api.addr_validate(&keeper)?;
    KEEPERS.save(deps.storage, &addr, &true)?;
    Ok(Response::new()
        .add_attribute("action", "add_keeper")
        .add_attribute("keeper", keeper))
}

fn remove_keeper(deps: DepsMut, info: MessageInfo, keeper: String) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }
    let addr = deps.api.addr_validate(&keeper)?;
    KEEPERS.remove(deps.storage, &addr);
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
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Pre-check the contract's actual balance so an over-requested
    // withdraw returns a clean `InsufficientBalance` with both values
    // visible, rather than a generic chain-side bank failure.
    let actual = deps
        .querier
        .query_balance(&env.contract.address, &denom)?
        .amount;
    let withdraw_amount = match amount {
        Some(a) => {
            if a > actual {
                return Err(ContractError::InsufficientBalance {
                    requested: a.to_string(),
                    balance: actual.to_string(),
                });
            }
            a
        }
        None => actual,
    };
    if withdraw_amount.is_zero() {
        return Err(ContractError::ZeroBalance {});
    }

    let to = match recipient {
        Some(r) => deps.api.addr_validate(&r)?.to_string(),
        None => cfg.owner.to_string(),
    };
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
        QueryMsg::Keepers { start_after, limit } => {
            to_json_binary(&query_keepers(deps, start_after, limit)?)
        }
        QueryMsg::PreviewBridge {} => to_json_binary(&query_preview_bridge(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner.to_string(),
        usdc_denom: cfg.usdc_denom,
        ibc_channel: cfg.ibc_channel,
        ibc_timeout_secs: cfg.ibc_timeout_secs,
        first_hop_receiver: cfg.first_hop_receiver,
        forward_memo: cfg.forward_memo,
        max_bridge_per_tx: cfg.max_bridge_per_tx,
        paused: cfg.paused,
    })
}

fn query_usdc_balance(deps: Deps, env: Env) -> StdResult<BalanceResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let amount = usdc_balance(&deps, &env, &cfg.usdc_denom)?;
    Ok(BalanceResponse { denom: cfg.usdc_denom, amount })
}

fn query_keepers(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<KeepersResponse> {
    let limit = limit
        .unwrap_or(DEFAULT_KEEPER_LIMIT)
        .min(MAX_KEEPER_LIMIT) as usize;

    // Validate-then-borrow the bech32 so the `Bound` lifetime ties to a
    // local Addr we own for the duration of the iterator.
    let start_after_addr = start_after
        .as_deref()
        .map(|s| deps.api.addr_validate(s))
        .transpose()?;
    let start = start_after_addr.as_ref().map(Bound::exclusive);

    let keepers: StdResult<Vec<String>> = KEEPERS
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|k| k.map(|a: Addr| a.to_string()))
        .collect();
    Ok(KeepersResponse { keepers: keepers? })
}

fn query_preview_bridge(deps: Deps) -> StdResult<PreviewBridgeResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(PreviewBridgeResponse {
        channel: cfg.ibc_channel,
        denom: cfg.usdc_denom,
        receiver: cfg.first_hop_receiver,
        memo: cfg.forward_memo,
        timeout_secs: cfg.ibc_timeout_secs,
        max_bridge_per_tx: cfg.max_bridge_per_tx,
    })
}

// ── Migrate ───────────────────────────────────────────────────────────────────

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = cw2::get_contract_version(deps.storage)?;
    if contract_version.contract != CONTRACT_NAME {
        return Err(ContractError::MigrationError {});
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

/// Reply handler for the outbound IBC submsg.
///
/// We use `reply_always`, so this fires on both success (packet queued by
/// the IBC module) and failure (queue rejected — channel closed, fee
/// insufficient, etc.). It does NOT see asynchronous ack-failure or
/// timeout — those land via separate IBC hooks which this contract
/// doesn't implement. Indexers downstream should treat
/// `ibc_transfer_status=queued` as "in flight", not "delivered".
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != BRIDGE_REPLY_ID {
        return Ok(Response::default());
    }

    // Always consume + clear the stashed context so the storage slot
    // doesn't leak across calls. `may_load` tolerates the (shouldn't-
    // happen) case where bridge_usdc didn't save one.
    let attempt = LAST_BRIDGE_ATTEMPT.may_load(deps.storage)?;
    LAST_BRIDGE_ATTEMPT.remove(deps.storage);

    let mut attrs = vec![attr("action", "bridge_reply")];
    if let Some(a) = attempt {
        attrs.push(attr("bridge_amount", a.amount));
        attrs.push(attr("bridge_receiver", a.receiver));
        attrs.push(attr("bridge_channel", a.channel));
        attrs.push(attr("bridge_denom", a.denom));
    }

    match msg.result {
        SubMsgResult::Ok(_) => {
            attrs.push(attr("ibc_transfer_status", "queued"));
        }
        SubMsgResult::Err(reason) => {
            attrs.push(attr("ibc_transfer_status", "failed"));
            attrs.push(attr("ibc_transfer_error", reason));
        }
    }

    Ok(Response::new().add_attributes(attrs))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn only_keeper_or_owner(cfg: &Config, deps: Deps, info: &MessageInfo) -> Result<(), ContractError> {
    if info.sender == cfg.owner || KEEPERS.has(deps.storage, &info.sender) {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}

fn usdc_balance(deps: &Deps, env: &Env, denom: &str) -> StdResult<Uint128> {
    Ok(deps.querier.query_balance(&env.contract.address, denom)?.amount)
}

/// Reject empty / whitespace-only first-hop receivers. We cannot
/// `addr_validate` here because the receiver lives on a different chain
/// (Noble), and Zigchain's API can only validate `zig1...` bech32.
fn validate_first_hop_receiver(receiver: &str) -> Result<(), ContractError> {
    if receiver.trim().is_empty() {
        return Err(ContractError::InvalidReceiver {});
    }
    Ok(())
}

/// Reject a zero-valued bridge cap. A zero cap would make every
/// `BridgeUsdc` call fail with `BridgeCapExceeded`, which is what
/// `SetPaused` is for. The explicit error keeps the operator from
/// accidentally creating a misleading state where the contract appears
/// usable but every call rejects.
fn validate_bridge_cap(cap: Uint128) -> Result<(), ContractError> {
    if cap.is_zero() {
        return Err(ContractError::InvalidBridgeCap {});
    }
    Ok(())
}

/// Best-effort validation of `forward_memo`. The memo is opaque to this
/// contract — it's interpreted by PFM / Noble-CCTP middleware on remote
/// chains — but we can at least check that it parses as a JSON object so
/// that the owner can't accidentally save a half-pasted string.
///
/// We deliberately don't validate the *meaning* of the JSON. That's the
/// rabbit hole that leads to schema-validating PFM memos — every new
/// middleware version becomes a potential bypass.
fn validate_forward_memo(memo: &str) -> Result<(), ContractError> {
    let trimmed = memo.trim();
    if trimmed.is_empty() {
        return Err(ContractError::InvalidMemo {});
    }
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(ContractError::InvalidMemo {});
    }
    // Walk the bytes verifying braces/quotes balance. Catches the typical
    // typo cases (missing closing brace, unterminated string) without
    // dragging serde_json into the contract.
    let bytes = trimmed.as_bytes();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    for &b in bytes {
        if escape {
            escape = false;
            continue;
        }
        match b {
            b'\\' if in_string => escape = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth < 0 {
                    return Err(ContractError::InvalidMemo {});
                }
            }
            _ => {}
        }
    }
    if depth != 0 || in_string {
        return Err(ContractError::InvalidMemo {});
    }
    Ok(())
}

/// Build a CosmosMsg::Stargate for `ibc.applications.transfer.v1.MsgTransfer`.
///
/// cosmwasm-std 1.5 `IbcMsg::Transfer` has no memo field, so we hand-encode
/// the protobuf directly. The memo carries Skip's PFM forwarding instructions
/// or Noble's CCTP routing — opaque to this contract.
///
/// # SAFETY: protobuf wire format is hand-rolled
///
/// The encoding below mirrors `ibc-go/modules/apps/transfer/types/tx.proto`
/// (the canonical `MsgTransfer` definition, stable since ibc-go v1).
/// Reference: <https://github.com/cosmos/ibc-go/blob/main/modules/apps/transfer/types/tx.proto>
///
/// Tag map used here:
///   1 source_port    (string)   — always "transfer"
///   2 source_channel (string)
///   3 token          (Coin)     — nested message: denom=1, amount=2
///   4 sender         (string)
///   5 receiver       (string)
///   6 timeout_height (Height)   — OMITTED; we use timestamp-only timeout
///   7 timeout_timestamp (uint64, nanoseconds)
///   8 memo           (string)   — optional, omitted when empty
///
/// If the upstream proto ever changes (re-numbered fields, new required
/// fields, additional wrappers), this function will silently produce a
/// malformed payload and the IBC module will reject the message. Any
/// audit/upgrade touching `MsgTransfer` MUST re-verify this encoder
/// against the live proto.
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
    pb_string(&mut buf, 1, "transfer");
    pb_string(&mut buf, 2, channel_id);
    let mut coin_buf = Vec::new();
    pb_string(&mut coin_buf, 1, denom);
    pb_string(&mut coin_buf, 2, &amount.to_string());
    pb_bytes(&mut buf, 3, &coin_buf);
    pb_string(&mut buf, 4, sender);
    pb_string(&mut buf, 5, receiver);
    // field 6 (timeout_height) deliberately omitted — timestamp-only timeout
    pb_uint64(&mut buf, 7, timeout_timestamp_nanos);
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
