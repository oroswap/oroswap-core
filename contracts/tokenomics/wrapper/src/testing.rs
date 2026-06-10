use crate::contract::{execute, CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::migrate::migrate;
use crate::msg::{ExecuteMsg, MigrateMsg};
use crate::state::{Config, DenomMetadata, DENOM_METADATA};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coin, Addr};

/// Construct a `Config` with the given owner and a synthetic mint denom,
/// skipping the create-denom reply path (which requires a proto encoded
/// response that the cosmwasm-std mock can't produce).
fn seed_config(deps: &mut cosmwasm_std::OwnedDeps<impl cosmwasm_std::Storage, impl cosmwasm_std::Api, impl cosmwasm_std::Querier>, owner: &str, lock_denom: &str, mint_denom: &str) {
    crate::state::CONFIG
        .save(
            &mut deps.storage,
            &Config {
                lock_denom: lock_denom.to_string(),
                mint_denom: mint_denom.to_string(),
                mint_decimals: 6,
                owner: Addr::unchecked(owner),
                paused: false,
                // Generous cap so existing unit tests aren't gated by it;
                // the dedicated `test_lock_rejects_when_supply_cap_hit`
                // exercises the cap explicitly.
                max_supply: cosmwasm_std::Uint128::new(1_000_000_000_000_000),
            },
        )
        .unwrap();
}

#[test]
fn test_lock_unlock() {
    let mut deps = mock_dependencies();
    let lock_denom = "uzig";
    let mint_denom = "coin.contract.wzig";
    seed_config(&mut deps, "owner", lock_denom, mint_denom);

    // Lock emits the mint sub-message
    let info = mock_info("user1", &[coin(1_000, lock_denom)]);
    let res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Lock { receiver: None }).unwrap();
    assert_eq!(res.messages.len(), 1);

    // Unlock emits burn + bank send
    let info = mock_info("user1", &[coin(500, mint_denom)]);
    let res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Unlock { receiver: None }).unwrap();
    assert_eq!(res.messages.len(), 2);
}

#[test]
fn test_pause_unpause() {
    let mut deps = mock_dependencies();
    let lock_denom = "uzig";
    let mint_denom = "coin.contract.wzig";
    seed_config(&mut deps, "owner", lock_denom, mint_denom);

    // Owner pauses
    let owner_info = mock_info("owner", &[]);
    execute(deps.as_mut(), mock_env(), owner_info.clone(), ExecuteMsg::Pause {}).unwrap();

    // Lock fails while paused
    let user_info = mock_info("user1", &[coin(1_000, lock_denom)]);
    let err = execute(deps.as_mut(), mock_env(), user_info, ExecuteMsg::Lock { receiver: None })
        .unwrap_err();
    assert!(err.to_string().contains("Contract is paused"));

    // Owner unpauses and lock succeeds
    execute(deps.as_mut(), mock_env(), owner_info, ExecuteMsg::Unpause {}).unwrap();
    let user_info = mock_info("user1", &[coin(1_000, lock_denom)]);
    execute(deps.as_mut(), mock_env(), user_info, ExecuteMsg::Lock { receiver: None }).unwrap();
}

#[test]
fn test_migrate_same_name() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, CONTRACT_VERSION).unwrap();

    let res = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let new_name = res
        .attributes
        .iter()
        .find(|a| a.key == "new_contract_name")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(new_name, CONTRACT_NAME);
}

/// Regression guard for the patch-semantics bug: an `UpdateDenomMetadata`
/// call that supplies only `uri` must NOT silently overwrite the previously
/// stored name/symbol/description with subdenom-derived defaults.
///
/// (Post v5-freeze trim: only description and uri remain in the variant; the
/// name and symbol fields are immutable post-instantiate and the test now
/// asserts they remain untouched across any update call.)
#[cfg(feature = "zigchain")]
#[test]
fn test_update_metadata_is_a_patch_not_a_reset() {
    let mut deps = mock_dependencies();
    seed_config(&mut deps, "owner", "uzig", "coin.contract.wzig");
    DENOM_METADATA
        .save(
            &mut deps.storage,
            &DenomMetadata {
                name: "Custom Wrapped Branding".to_string(),
                symbol: "wBRAND".to_string(),
                description: "Hand-picked description".to_string(),
                uri: Some("https://old.example/logo.png".to_string()),
                uri_hash: Some("oldhash".to_string()),
            },
        )
        .unwrap();

    // "Just update the logo" — everything else should be untouched.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::UpdateDenomMetadata {
            description: None,
            uri: Some("https://new.example/logo.png".to_string()),
            uri_hash: None,
        },
    )
    .unwrap();

    let stored = DENOM_METADATA.load(&deps.storage).unwrap();
    assert_eq!(stored.name, "Custom Wrapped Branding");
    assert_eq!(stored.symbol, "wBRAND");
    assert_eq!(stored.description, "Hand-picked description");
    assert_eq!(stored.uri.as_deref(), Some("https://new.example/logo.png"));
    assert_eq!(stored.uri_hash.as_deref(), Some("oldhash"));
}

/// Patching a single editable field overlays only that field; the rest
/// (including the v5-frozen identity fields) are preserved.
#[cfg(feature = "zigchain")]
#[test]
fn test_update_metadata_overlays_supplied_fields() {
    let mut deps = mock_dependencies();
    seed_config(&mut deps, "owner", "uzig", "coin.contract.wzig");
    DENOM_METADATA
        .save(
            &mut deps.storage,
            &DenomMetadata {
                name: "Original Name".to_string(),
                symbol: "ORIG".to_string(),
                description: "Original description".to_string(),
                uri: Some("https://example/old.png".to_string()),
                uri_hash: None,
            },
        )
        .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::UpdateDenomMetadata {
            description: Some("New description".to_string()),
            uri: None,
            uri_hash: Some("freshhash".to_string()),
        },
    )
    .unwrap();

    let stored = DENOM_METADATA.load(&deps.storage).unwrap();
    assert_eq!(stored.name, "Original Name");
    assert_eq!(stored.symbol, "ORIG");
    assert_eq!(stored.description, "New description");
    assert_eq!(stored.uri.as_deref(), Some("https://example/old.png"));
    assert_eq!(stored.uri_hash.as_deref(), Some("freshhash"));
}

#[test]
fn test_migrate_wrong_name_rejected() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "some-other-contract", "0.0.1").unwrap();

    let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
    // FIND-007 fix: migrate now delegates the name guard to
    // cw2::ensure_from_older_version, which surfaces a generic StdError
    // wrapped into ContractError::Std on mismatch.
    assert!(
        matches!(err, ContractError::Std(_)),
        "expected wrapped StdError on name mismatch, got {err:?}"
    );
}

// ─── ProposeDenomAdmin guards ────────────────────────────────────────────────
//
// `ProposeDenomAdmin` is the highest-blast-radius owner-only handler in the
// contract (hands off bank/metadata admin of the minted denom to an
// external address). Audit M-A flagged the lack of test coverage. Each
// guard below is exercised in isolation.

#[cfg(feature = "zigchain")]
#[test]
fn test_propose_denom_admin_non_owner_rejected() {
    let mut deps = mock_dependencies();
    seed_config(&mut deps, "owner", "uzig", "coin.contract.wzig");

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("not-owner", &[]),
        ExecuteMsg::ProposeDenomAdmin {
            bank_admin: "zig1bankadmin000000000000000000000000000000".to_string(),
            metadata_admin: "zig1metaadmin000000000000000000000000000000".to_string(),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

/// Reply hasn't yet populated `mint_denom`: the chain-not-yet-initialised
/// guard must reject the call rather than send `MsgProposeDenomAdmin` for
/// an empty denom (which the chain would either silently drop or apply to
/// the wrong/no token).
#[cfg(feature = "zigchain")]
#[test]
fn test_propose_denom_admin_empty_mint_denom_rejected() {
    let mut deps = mock_dependencies();
    // Seed with empty mint_denom — mirrors the state between MsgCreateDenom
    // submsg dispatch and the reply that fills mint_denom in.
    seed_config(&mut deps, "owner", "uzig", "");

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::ProposeDenomAdmin {
            bank_admin: "zig1bankadmin000000000000000000000000000000".to_string(),
            metadata_admin: "zig1metaadmin000000000000000000000000000000".to_string(),
        },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("mint_denom not yet initialised"),
        "expected init-guard error, got: {err}"
    );
}

#[cfg(feature = "zigchain")]
#[test]
fn test_propose_denom_admin_invalid_bech32_rejected() {
    let mut deps = mock_dependencies();
    seed_config(&mut deps, "owner", "uzig", "coin.contract.wzig");

    // Empty bank_admin — addr_validate refuses.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::ProposeDenomAdmin {
            bank_admin: "".to_string(),
            metadata_admin: "zig1metaadmin000000000000000000000000000000".to_string(),
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, ContractError::Std(_)),
        "expected Std (addr_validate) error, got: {err:?}"
    );

    // Empty metadata_admin — addr_validate refuses on the second call.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("owner", &[]),
        ExecuteMsg::ProposeDenomAdmin {
            bank_admin: "zig1bankadmin000000000000000000000000000000".to_string(),
            metadata_admin: "".to_string(),
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, ContractError::Std(_)),
        "expected Std (addr_validate) error, got: {err:?}"
    );
}

/// FIND-013 regression: the InstantiateDenom reply handler must refuse to
/// persist an empty mint_denom. Protobuf decode can succeed while leaving
/// the string empty (mismatched feature flags, future schema drift).
/// Without the guard, the contract would store an unusable denom and every
/// subsequent Lock/Unlock would fail.
#[test]
fn reply_rejects_empty_mint_denom() {
    use crate::contract::reply;
    use cosmwasm_std::{Binary, Reply, SubMsgResponse, SubMsgResult};
    use oroswap::token_factory::MsgCreateDenomResponse;

    let mut deps = mock_dependencies();

    // Encode the zigchain-variant MsgCreateDenomResponse with denom = "".
    // (The wrapper's default feature set includes "zigchain", so the reply
    // handler decodes this 8-field struct.)
    #[allow(non_snake_case)]
    let proto: Binary = MsgCreateDenomResponse {
        creator: String::new(),
        bank_admin: String::new(),
        metadata_admin: String::new(),
        denom: String::new(),
        minting_cap: String::new(),
        can_change_minting_cap: false,
        URI: String::new(),
        URI_hash: String::new(),
    }
    .into();

    let reply_msg = Reply {
        id: 1, // ReplyIds::InstantiateDenom
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(proto),
        }),
    };

    let err = reply(deps.as_mut(), mock_env(), reply_msg).unwrap_err();
    assert!(
        matches!(err, ContractError::FailedToParseReply {}),
        "empty mint_denom must surface as FailedToParseReply, got {err:?}",
    );
}

/// FIND-005 regression: `mint_max_supply == 0` must be rejected at
/// instantiate. The cap is immutable on-chain (`can_change_minting_cap: false`),
/// so a zero cap would silently brick all future mints with no recovery path.
#[test]
fn instantiate_rejects_zero_mint_max_supply() {
    use crate::contract::instantiate;
    use crate::msg::InstantiateMsg;
    use cosmwasm_std::Uint128;

    let mut deps = mock_dependencies();

    let err = instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg {
            lock_denom: "uzig".to_string(),
            mint_subdenom: "wzig".to_string(),
            mint_uri: None,
            mint_uri_hash: None,
            mint_decimals: 6,
            mint_name: None,
            mint_symbol: None,
            mint_description: None,
            owner: "owner".to_string(),
            mint_max_supply: Uint128::zero(),
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, ContractError::InvalidMintMaxSupply {}),
        "zero mint_max_supply must surface as InvalidMintMaxSupply, got {err:?}",
    );
}
