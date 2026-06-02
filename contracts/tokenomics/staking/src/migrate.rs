#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response};
use cw2::ensure_from_older_version;

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::state::{REWARD_ORO_RESERVE, TOTAL_LOCKED};
use oroswap::staking::MigrateMsg;
use cosmwasm_std::Uint128;

/// Forward-only migration entry point.
///
/// `ensure_from_older_version` validates contract name parity and rejects
/// downgrades (storage_version > CONTRACT_VERSION). Same-version migrations
/// are silent no-ops per cw2 convention. The handler then initialises any
/// state items added in later versions so subsequent execute paths can
/// rely on them without panicking on `load()`.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let stored_version = ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Ensure new state items exist (idempotent — safe to run on any version).
    if REWARD_ORO_RESERVE.may_load(deps.storage)?.is_none() {
        REWARD_ORO_RESERVE.save(deps.storage, &Uint128::zero())?;
    }
    if TOTAL_LOCKED.may_load(deps.storage)?.is_none() {
        TOTAL_LOCKED.save(deps.storage, &0u128)?;
    }

    Ok(Response::new()
        .add_attribute("previous_contract_name", CONTRACT_NAME)
        .add_attribute("previous_contract_version", stored_version.to_string())
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}

#[cfg(test)]
mod tests {
    //! Migration tests that exercise the legacy-bytes deserialisation path.
    //!
    //! These cannot be expressed in `cw-multi-test`: the wasm keeper
    //! namespaces contract storage behind the contract address and
    //! `App::init_modules(...)` writes go to the top-level test keyspace,
    //! so any "legacy state" planted that way is invisible to the migrate
    //! handler. Here we use `mock_dependencies()` and write the legacy
    //! bytes directly into the contract's own storage. (Audit N-2.)
    use super::*;
    use cosmwasm_schema::cw_serde;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cw_storage_plus::Item;
    use oroswap::staking::Config;

    /// Pre-2.2.0 Config shape: identical to current `Config` minus the
    /// `paused` field. Mirrors what would actually be sitting on a chain
    /// that was deployed before pause-control was added.
    #[cw_serde]
    struct LegacyConfig {
        pub oro_denom: String,
        pub xoro_denom: String,
        pub bootstrap_amount: cosmwasm_std::Uint128,
        pub owner: String,
        pub unbonding_period: u64,
        pub xoro_decimals: u32,
        pub xoro_uri: Option<String>,
    }

    fn seed_legacy_state(deps: &mut cosmwasm_std::OwnedDeps<impl cosmwasm_std::Storage, impl cosmwasm_std::Api, impl cosmwasm_std::Querier>) {
        // Plant the legacy Config under the exact storage key the contract
        // uses (`"config"`) so post-migrate CONFIG.load() reads it back.
        let legacy_item: Item<LegacyConfig> = Item::new("config");
        legacy_item
            .save(
                &mut deps.storage,
                &LegacyConfig {
                    oro_denom: "coin.assembly.oro".to_string(),
                    xoro_denom: "coin.contract.xoro".to_string(),
                    bootstrap_amount: cosmwasm_std::Uint128::new(1_000),
                    owner: "owner".to_string(),
                    unbonding_period: 604_800,
                    xoro_decimals: 6,
                    xoro_uri: Some("https://example.com".to_string()),
                },
            )
            .unwrap();
        cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, "2.1.0").unwrap();
    }

    /// The load-bearing test: legacy bytes (no `paused` field) must
    /// deserialise into the new Config with `paused = false` after
    /// migrate. This passes only because of `#[serde(default)] paused`.
    #[test]
    fn migrate_from_legacy_config_deserialises_with_default_paused() {
        let mut deps = mock_dependencies();
        seed_legacy_state(&mut deps);

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();

        // The current Config has the extra `paused` field. Load must succeed
        // because serde fills it with the default (`false`).
        let cfg: Config = Item::<Config>::new("config").load(&deps.storage).unwrap();
        assert_eq!(cfg.oro_denom, "coin.assembly.oro");
        assert_eq!(cfg.unbonding_period, 604_800);
        assert!(!cfg.paused, "missing `paused` in legacy bytes must default to false");
    }

    /// Migrate must initialise any state items added in later versions
    /// (`REWARD_ORO_RESERVE`, `TOTAL_LOCKED`) so subsequent execute paths
    /// can rely on them without panicking on `load()`.
    #[test]
    fn migrate_initialises_new_state_items() {
        let mut deps = mock_dependencies();
        seed_legacy_state(&mut deps);
        // Sanity: items absent pre-migrate.
        assert!(REWARD_ORO_RESERVE.may_load(&deps.storage).unwrap().is_none());
        assert!(TOTAL_LOCKED.may_load(&deps.storage).unwrap().is_none());

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();

        assert_eq!(
            REWARD_ORO_RESERVE.load(&deps.storage).unwrap(),
            cosmwasm_std::Uint128::zero(),
        );
        assert_eq!(TOTAL_LOCKED.load(&deps.storage).unwrap(), 0u128);
    }

    #[test]
    fn migrate_idempotent_when_state_already_present() {
        let mut deps = mock_dependencies();
        seed_legacy_state(&mut deps);
        // Pre-populate the "new" state items with non-zero values to confirm
        // migrate doesn't clobber them.
        REWARD_ORO_RESERVE
            .save(&mut deps.storage, &cosmwasm_std::Uint128::new(42))
            .unwrap();
        TOTAL_LOCKED.save(&mut deps.storage, &99u128).unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();

        assert_eq!(
            REWARD_ORO_RESERVE.load(&deps.storage).unwrap(),
            cosmwasm_std::Uint128::new(42),
            "migrate must NOT overwrite an already-initialised reserve",
        );
        assert_eq!(TOTAL_LOCKED.load(&deps.storage).unwrap(), 99u128);
    }

    #[test]
    fn migrate_rejects_foreign_contract_name() {
        let mut deps = mock_dependencies();
        cw2::set_contract_version(&mut deps.storage, "some-other-contract", "0.0.1").unwrap();

        let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
        // ensure_from_older_version surfaces a generic StdError on mismatch,
        // wrapped into ContractError::Std by the `?` operator.
        assert!(
            matches!(err, ContractError::Std(_)),
            "expected wrapped StdError on name mismatch, got {err:?}"
        );
    }

    /// Audit FIND-007 regression: migrating to an OLDER version must be
    /// rejected so a stale wasm code id can't accidentally roll the
    /// contract back. Storage version must remain unchanged on failure.
    #[test]
    fn migrate_rejects_downgrade() {
        let mut deps = mock_dependencies();
        // Seed a stored version strictly higher than CONTRACT_VERSION (currently 2.2.0).
        cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, "99.0.0").unwrap();

        let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
        assert!(
            matches!(err, ContractError::Std(_)),
            "expected wrapped StdError on downgrade, got {err:?}"
        );

        // State items must NOT have been initialised on the rejected path,
        // and the stored version must not have moved.
        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, "99.0.0", "downgrade must not mutate stored version");
        assert!(
            REWARD_ORO_RESERVE.may_load(&deps.storage).unwrap().is_none(),
            "state-init must not run on a rejected migration",
        );
    }

    /// Belt-and-braces against a future Cargo.toml bump to a non-semver
    /// string (e.g. "1.0", "v1.0.0", "latest"). `ensure_from_older_version`
    /// parses CONTRACT_VERSION via the semver crate at runtime; a bad bump
    /// would brick every on-chain migrate including the first one. This
    /// test catches it at `cargo test` time instead — and doubles as a
    /// positive-path upgrade test from a synthetic 0.0.1 baseline.
    #[test]
    fn migrate_upgrade_from_older_version_succeeds_and_parses_semver() {
        let mut deps = mock_dependencies();
        cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, "0.0.1").unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {})
            .expect("CONTRACT_VERSION must be strict semver and accept an older stored version");
        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, CONTRACT_VERSION, "stored version must advance on forward migration");
        // State-init must have run on the upgrade path.
        assert_eq!(
            REWARD_ORO_RESERVE.load(&deps.storage).unwrap(),
            cosmwasm_std::Uint128::zero(),
        );
        assert_eq!(TOTAL_LOCKED.load(&deps.storage).unwrap(), 0u128);
    }

    /// Same-version migration is a no-op per cw2 convention but the handler
    /// must still complete successfully and run the idempotent state-init
    /// so newly-added items are guaranteed present.
    #[test]
    fn migrate_accepts_same_version_as_noop() {
        let mut deps = mock_dependencies();
        cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, CONTRACT_VERSION).unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();

        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, CONTRACT_VERSION);
        // State items still initialised.
        assert_eq!(
            REWARD_ORO_RESERVE.load(&deps.storage).unwrap(),
            cosmwasm_std::Uint128::zero(),
        );
        assert_eq!(TOTAL_LOCKED.load(&deps.storage).unwrap(), 0u128);
    }
}
