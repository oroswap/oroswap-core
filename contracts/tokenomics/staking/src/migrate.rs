//! Forward-only migration scaffolding: validates contract-name parity and
//! rejects downgrades via cw2, then advances the stored version.

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response};
use cw2::ensure_from_older_version;

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use oroswap::staking::MigrateMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let stored_version = ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("from", stored_version.to_string())
        .add_attribute("to", CONTRACT_VERSION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};

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
        // Seed a stored version strictly higher than CONTRACT_VERSION.
        cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, "99.0.0").unwrap();

        let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
        assert!(
            matches!(err, ContractError::Std(_)),
            "expected wrapped StdError on downgrade, got {err:?}"
        );

        // Stored version must not have moved on the rejected path.
        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, "99.0.0", "downgrade must not mutate stored version");
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
    }

    /// Same-version migration is a no-op per cw2 convention but the handler
    /// must still complete successfully.
    #[test]
    fn migrate_accepts_same_version_as_noop() {
        let mut deps = mock_dependencies();
        cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, CONTRACT_VERSION).unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();

        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, CONTRACT_VERSION);
    }
}
