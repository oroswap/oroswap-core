#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response};
use cw2::ensure_from_older_version;

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::msg::MigrateMsg;

/// Forward-only migration. `ensure_from_older_version` validates contract
/// name parity AND rejects downgrades: storage_version > CONTRACT_VERSION
/// errors out, while storage_version == CONTRACT_VERSION is a silent no-op
/// (per cw2 convention). Prevents accidental rollbacks via stale wasm code
/// IDs without changing the admin's migration authority.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let stored_version = ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("previous_contract_name", CONTRACT_NAME)
        .add_attribute("previous_contract_version", stored_version.to_string())
        .add_attribute("new_contract_name", CONTRACT_NAME)
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cw2::set_contract_version;

    #[test]
    fn migrate_rejects_foreign_contract_name() {
        let mut deps = mock_dependencies();
        set_contract_version(&mut deps.storage, "some-other-contract", "0.0.1").unwrap();

        let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
        assert!(
            matches!(err, ContractError::Std(_)),
            "expected wrapped StdError on name mismatch, got {err:?}"
        );
    }

    #[test]
    fn migrate_rejects_downgrade() {
        let mut deps = mock_dependencies();
        set_contract_version(&mut deps.storage, CONTRACT_NAME, "99.0.0").unwrap();

        let err = migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap_err();
        assert!(
            matches!(err, ContractError::Std(_)),
            "expected wrapped StdError on downgrade, got {err:?}"
        );
        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, "99.0.0", "downgrade must not mutate stored version");
    }

    #[test]
    fn migrate_accepts_same_version_as_noop() {
        let mut deps = mock_dependencies();
        set_contract_version(&mut deps.storage, CONTRACT_NAME, CONTRACT_VERSION).unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, CONTRACT_VERSION);
    }

    /// Belt-and-braces against a future Cargo.toml bump to a non-semver
    /// string (e.g. "1.0", "v1.0.0", "latest"). `ensure_from_older_version`
    /// parses CONTRACT_VERSION via the semver crate at runtime; a bad bump
    /// would brick every on-chain migrate including the first one. This
    /// test catches it at `cargo test` time instead — and doubles as the
    /// positive-path upgrade test (older stored → bumped to CONTRACT_VERSION).
    #[test]
    fn migrate_upgrade_from_older_version_succeeds_and_parses_semver() {
        let mut deps = mock_dependencies();
        set_contract_version(&mut deps.storage, CONTRACT_NAME, "0.0.1").unwrap();

        migrate(deps.as_mut(), mock_env(), MigrateMsg {})
            .expect("CONTRACT_VERSION must be strict semver and accept an older stored version");
        let stored = cw2::get_contract_version(&deps.storage).unwrap();
        assert_eq!(stored.version, CONTRACT_VERSION, "stored version must advance on forward migration");
    }
}
