#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response};
use cw2::{get_contract_version, set_contract_version};

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::state::{REWARD_ORO_RESERVE, TOTAL_LOCKED};
use oroswap::staking::MigrateMsg;
use cosmwasm_std::Uint128;

/// Migration entry point.
///
/// This contract is freshly deployed — no legacy state to migrate.
/// The handler initialises any state items that may be absent on older
/// deployments and bumps the stored contract version.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;

    // Only allow migrating from the same contract name.
    if contract_version.contract != CONTRACT_NAME {
        return Err(ContractError::MigrationError {});
    }

    // Ensure new state items exist (idempotent — safe to run on any version).
    if REWARD_ORO_RESERVE.may_load(deps.storage)?.is_none() {
        REWARD_ORO_RESERVE.save(deps.storage, &Uint128::zero())?;
    }
    if TOTAL_LOCKED.may_load(deps.storage)?.is_none() {
        TOTAL_LOCKED.save(deps.storage, &0u128)?;
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
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
        assert!(matches!(err, ContractError::MigrationError {}));
    }
}
