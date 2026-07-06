use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Deps, Order, StdResult};
use cw_storage_plus::{Bound, Item, Map};
use oroswap::asset::AssetInfo;
use oroswap::factory::{default_pool_creation_fee_denom, Config, PairConfig, TrackerConfig, PairType, StartAfter};
use oroswap::common::OwnershipProposal;

#[cfg(test)]
use cosmwasm_std::testing::MockApi;


use crate::error::ContractError;
/// This is an intermediate structure for storing a pair's key. It is used in a submessage response.
#[cw_serde]
pub struct TmpPairInfo {
    pub pair_key: Vec<u8>,
}

/// Saves a pair's key
pub const TMP_PAIR_INFO: Item<TmpPairInfo> = Item::new("tmp_pair_info");

/// Saves factory settings
pub const CONFIG: Item<Config> = Item::new("config");

/// Native denom accepted (and forwarded to the maker) as the pool-creation fee.
/// Stored separately from [`Config`] so the shared config struct other contracts
/// deserialize isn't bloated by a factory-only setting. The raw storage key
/// (`pool_creation_fee_denom`) is read directly by the pair at instantiate via
/// `query_factory_pool_creation_fee_denom`.
pub const POOL_CREATION_FEE_DENOM: Item<String> = Item::new("pool_creation_fee_denom");

/// Loads the pool-creation fee denom, falling back to the historical default
/// (`uzig`) when unset — e.g. a factory deployed before this key existed.
pub fn load_pool_creation_fee_denom(storage: &dyn cosmwasm_std::Storage) -> StdResult<String> {
    Ok(POOL_CREATION_FEE_DENOM
        .may_load(storage)?
        .unwrap_or_else(default_pool_creation_fee_denom))
}

/// Saves created pairs (from olders to latest)
pub const PAIRS: Map<&[u8], Addr> = Map::new("pair_info");

/// Track config for tracking contract
pub const TRACKER_CONFIG: Item<TrackerConfig> = Item::new("tracker_config");

/// Calculates a pair key from the specified parameters in the `asset_infos` variable.
///
/// `asset_infos` is an array with multiple items of type [`AssetInfo`].
pub fn pair_key(asset_infos: &[AssetInfo], pair_type: &PairType) -> Vec<u8> {
    let mut key = Vec::<u8>::new();
    
    // Sort asset infos by their bytes to ensure consistent ordering
    let mut sorted_assets = asset_infos.to_vec();
    sorted_assets.sort_by(|a, b| a.as_bytes().cmp(&b.as_bytes()));
    
    // Add all asset infos with delimiters
    for (i, asset) in sorted_assets.iter().enumerate() {
        if i > 0 {
            // Add delimiter between assets to prevent collisions
            // Using \x01 (start of heading) as it cannot appear in token names
            key.extend_from_slice(b"\x01");
        }
        key.extend_from_slice(&asset.as_bytes());
    }
    
    // Add delimiter before pair type
    key.extend_from_slice(b"\x01");
    
    // Append pair type to the key
    key.extend_from_slice(pair_type.to_string().as_bytes());
    key
}

/// Rebuilds a pair key produced by [`pair_key`] with the native denom `from`
/// swapped to `to`. Returns `Some(new_key)` if `from` appeared as one of the
/// key's asset segments, or `None` if the key is unaffected (so callers skip it,
/// making a redenomination migrate idempotent).
///
/// A key is `asset_bytes` (sorted, `\x01`-delimited) followed by `\x01` and the
/// pair-type string. `\x01` cannot appear inside a valid native denom (see
/// [`validate_native_denom`]) or a bech32 address, so splitting on it is safe;
/// the last segment is always the pair type. Because `from` and `to` can sort
/// differently (e.g. `azig` < `uzig`), the asset segments are re-sorted after
/// the swap — reproducing exactly what [`pair_key`] would emit for the swapped
/// assets, which a naive byte substring-replace would not.
pub fn swap_denom_in_pair_key(old_key: &[u8], from: &[u8], to: &[u8]) -> Option<Vec<u8>> {
    let mut segments: Vec<Vec<u8>> = old_key
        .split(|b| *b == 0x01u8)
        .map(|s| s.to_vec())
        .collect();

    // A well-formed pair key has at least one asset segment plus the pair-type
    // segment. Anything shorter is not a pair key we should touch.
    if segments.len() < 2 {
        return None;
    }

    // The final segment is the pair type; everything before it is an asset.
    let pair_type = segments.pop().unwrap();

    let mut changed = false;
    for seg in segments.iter_mut() {
        if seg.as_slice() == from {
            *seg = to.to_vec();
            changed = true;
        }
    }
    if !changed {
        return None;
    }

    // Re-sort asset segments to match pair_key's byte ordering.
    segments.sort();

    let mut new_key = Vec::<u8>::new();
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            new_key.extend_from_slice(b"\x01");
        }
        new_key.extend_from_slice(seg);
    }
    new_key.extend_from_slice(b"\x01");
    new_key.extend_from_slice(&pair_type);
    Some(new_key)
}

/// Saves pair type configurations
pub const PAIR_CONFIGS: Map<String, PairConfig> = Map::new("pair_configs");

/// Saves paused pairs
pub const PAUSED_PAIRS: Map<&[u8], ()> = Map::new("paused_pairs");

/// Saves addresses with pause authority
pub const PAUSE_AUTHORITIES: Map<&Addr, ()> = Map::new("pause_authorities");

/// ## Pagination settings
/// The maximum limit for reading pairs from [`PAIRS`]
const MAX_LIMIT: u32 = 30;
/// The default limit for reading pairs from [`PAIRS`]
const DEFAULT_LIMIT: u32 = 10;

/// Reads pairs from the [`PAIRS`] vector according to the `start_after` and `limit` variables.
/// Otherwise, it returns the default number of pairs, starting from the oldest one.
///
/// `start_after` is the pair from which the function starts to fetch results.
///
/// `limit` is the number of items to retrieve.
pub fn read_pairs(
    deps: Deps,
    start_after: Option<StartAfter>,
    limit: Option<u32>,
) -> StdResult<Vec<Addr>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    if let Some(start) = calc_range_start(start_after) {
        PAIRS
            .range(
                deps.storage,
                Some(Bound::exclusive(start.as_slice())),
                None,
                Order::Ascending,
            )
            .take(limit)
            .map(|item| {
                let (_, pair_addr) = item?;
                Ok(pair_addr)
            })
            .collect()
    } else {
        PAIRS
            .range(deps.storage, None, None, Order::Ascending)
            .take(limit)
            .map(|item| {
                let (_, pair_addr) = item?;
                Ok(pair_addr)
            })
            .collect()
    }
}

/// Calculates the key of a pair from which to start reading data.
///
/// `start_after` is an [`Option`] type that contains both the asset infos and pair type
/// of the pair from which we start reading data.
fn calc_range_start(
    start_after: Option<StartAfter>,
) -> Option<Vec<u8>> {
    start_after.map(|start| {
        let mut key = pair_key(&start.asset_infos, &start.pair_type);
        key.push(1);
        key
    })
}

pub(crate) fn check_asset_infos(
    api: &dyn Api,
    asset_infos: &[AssetInfo],
) -> Result<(), ContractError> {
    // Check for duplicates with case-insensitive comparison
    for i in 0..asset_infos.len() {
        for j in (i + 1)..asset_infos.len() {
            if asset_infos[i].case_insensitive_eq(&asset_infos[j]) {
                return Err(ContractError::DoublingAssets {});
            }
        }
    }

    asset_infos
        .iter()
        .try_for_each(|asset_info| asset_info.check(api))
        .map_err(Into::into)
}

/// Stores the latest contract ownership transfer proposal
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");

/// This state key isn't used anymore but left for backward compatability with old pairs
pub const PAIRS_TO_MIGRATE: Item<Vec<Addr>> = Item::new("pairs_to_migrate");

#[cfg(test)]
mod tests {
    use oroswap::asset::{native_asset_info, token_asset_info};

    use super::*;

    fn get_test_case() -> Vec<[AssetInfo; 2]> {
        vec![
            [
                native_asset_info("uluna".to_string()),
                native_asset_info("uusd".to_string()),
            ],
            [
                native_asset_info("uluna".to_string()),
                token_asset_info(Addr::unchecked("oro_token_addr")),
            ],
            [
                token_asset_info(Addr::unchecked("random_token_addr")),
                token_asset_info(Addr::unchecked("oro_token_addr")),
            ],
        ]
    }

    #[test]
    fn test_pair_key() {
        // Test different pair types generate different keys
        let asset_infos = get_test_case()[0].clone();
        let xyk_key = pair_key(&asset_infos, &PairType::Xyk {});
        let stable_key = pair_key(&asset_infos, &PairType::Stable {});
        let custom_key = pair_key(&asset_infos, &PairType::Custom("xyk_30".to_string()));

        // Verify different pair types generate different keys
        assert_ne!(xyk_key, stable_key);
        assert_ne!(xyk_key, custom_key);
        assert_ne!(stable_key, custom_key);

        // Verify same pair type with same assets generates same key
        assert_eq!(
            pair_key(&asset_infos, &PairType::Xyk {}),
            pair_key(&asset_infos, &PairType::Xyk {})
        );

        // Verify key format includes pair type with delimiters
        let mut expected_key = Vec::<u8>::new();
        expected_key.extend_from_slice(&asset_infos[0].as_bytes());
        expected_key.extend_from_slice(b"\x01");  // delimiter between assets
        expected_key.extend_from_slice(&asset_infos[1].as_bytes());
        expected_key.extend_from_slice(b"\x01");  // delimiter before pair type
        expected_key.extend_from_slice(b"xyk");
        assert_eq!(xyk_key, expected_key);
    }

    #[test]
    fn test_case_insensitive_asset_check() {
        // Test that case-insensitive duplicates are detected
        let asset_infos = [
            AssetInfo::Token { contract_addr: Addr::unchecked("contract123") },
            AssetInfo::Token { contract_addr: Addr::unchecked("CONTRACT123") },
        ];
        
        // This should fail with DoublingAssets error
        let result = check_asset_infos(&MockApi::default(), &asset_infos);
        assert!(result.is_err());
        
        // Test with native tokens
        let native_asset_infos = [
            AssetInfo::NativeToken { denom: "uluna".to_string() },
            AssetInfo::NativeToken { denom: "ULUNA".to_string() },
        ];
        
        let result = check_asset_infos(&MockApi::default(), &native_asset_infos);
        assert!(result.is_err());
        
        // Test that different assets are allowed
        let different_asset_infos = [
            AssetInfo::Token { contract_addr: Addr::unchecked("contract123") },
            AssetInfo::Token { contract_addr: Addr::unchecked("different") },
        ];
        
        let result = check_asset_infos(&MockApi::default(), &different_asset_infos);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pair_key_collision_fix() {
        // Test that the collision issue is fixed for 2-token pairs
        // Create two different asset combinations that would collide without delimiters
        
        // Asset 1: "abc" + "def" 
        let asset_infos1 = [
            AssetInfo::NativeToken { denom: "abc".to_string() },
            AssetInfo::NativeToken { denom: "def".to_string() },
        ];
        
        // Asset 2: "ab" + "cdef" (this would collide with asset_infos1 without delimiters)
        let asset_infos2 = [
            AssetInfo::NativeToken { denom: "ab".to_string() },
            AssetInfo::NativeToken { denom: "cdef".to_string() },
        ];
        
        let pair_type = PairType::Xyk {};
        
        let key1 = pair_key(&asset_infos1, &pair_type);
        let key2 = pair_key(&asset_infos2, &pair_type);
        
        // Verify that the keys are different (collision is fixed)
        assert_ne!(key1, key2, "Key collision detected! Keys should be different with delimiters.");
        
        // Verify the keys have the expected format with delimiters
        let expected_key1 = {
            let mut key = Vec::<u8>::new();
            key.extend_from_slice(b"abc");
            key.extend_from_slice(b"\x01");  // delimiter between assets
            key.extend_from_slice(b"def");
            key.extend_from_slice(b"\x01");  // delimiter before pair type
            key.extend_from_slice(b"xyk");
            key
        };
        
        let expected_key2 = {
            let mut key = Vec::<u8>::new();
            key.extend_from_slice(b"ab");
            key.extend_from_slice(b"\x01");  // delimiter between assets
            key.extend_from_slice(b"cdef");
            key.extend_from_slice(b"\x01");  // delimiter before pair type
            key.extend_from_slice(b"xyk");
            key
        };
        
        assert_eq!(key1, expected_key1);
        assert_eq!(key2, expected_key2);
    }

    #[test]
    fn test_legacy_start_after() {
        fn new_calc_range_start(start_after: Option<[AssetInfo; 2]>) -> Option<Vec<u8>> {
            start_after.map(|asset_infos| {
                let mut asset_infos = asset_infos.to_vec();
                asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

                let mut v = Vec::<u8>::new();
                v.extend_from_slice(asset_infos[0].as_bytes());
                v.extend_from_slice(b"\x01"); // delimiter between assets
                v.extend_from_slice(asset_infos[1].as_bytes());
                v.extend_from_slice(b"\x01"); // delimiter before pair type
                v.extend_from_slice(b"xyk"); // Add pair type
                v.push(1); // Add the 1 at the end like calc_range_start does
                v
            })
        }

        for asset_infos in get_test_case() {
            assert_eq!(
                new_calc_range_start(Some(asset_infos.clone())),
                calc_range_start(Some(StartAfter {
                    asset_infos: asset_infos.to_vec(),
                    pair_type: PairType::Xyk {},
                }))
            );
        }
    }

    #[test]
    fn test_swap_denom_in_pair_key() {
        let token = token_asset_info(Addr::unchecked("token_addr"));

        // Swapping uzig->azig must equal pair_key rebuilt from the swapped assets.
        let uzig_assets = [native_asset_info("uzig".to_string()), token.clone()];
        let azig_assets = [native_asset_info("azig".to_string()), token.clone()];
        let uzig_key = pair_key(&uzig_assets, &PairType::Xyk {});
        let expected = pair_key(&azig_assets, &PairType::Xyk {});
        assert_eq!(
            swap_denom_in_pair_key(&uzig_key, b"uzig", b"azig").unwrap(),
            expected
        );

        // Idempotent: a key already on azig has no uzig segment -> None.
        assert!(swap_denom_in_pair_key(&expected, b"uzig", b"azig").is_none());

        // Unrelated key (no uzig) -> None.
        let control = pair_key(
            &[native_asset_info("uusd".to_string()), token.clone()],
            &PairType::Xyk {},
        );
        assert!(swap_denom_in_pair_key(&control, b"uzig", b"azig").is_none());

        // Two native denoms where the swap flips sort order (uosmo < uzig, but
        // azig < uosmo): the asset segments must be re-sorted, not substring-replaced.
        let a = [
            native_asset_info("uzig".to_string()),
            native_asset_info("uosmo".to_string()),
        ];
        let b = [
            native_asset_info("azig".to_string()),
            native_asset_info("uosmo".to_string()),
        ];
        assert_eq!(
            swap_denom_in_pair_key(&pair_key(&a, &PairType::Stable {}), b"uzig", b"azig").unwrap(),
            pair_key(&b, &PairType::Stable {})
        );

        // Custom pair type (string contains '-', never \x01) is preserved.
        assert_eq!(
            swap_denom_in_pair_key(
                &pair_key(&uzig_assets, &PairType::Custom("concentrated".to_string())),
                b"uzig",
                b"azig"
            )
            .unwrap(),
            pair_key(&azig_assets, &PairType::Custom("concentrated".to_string()))
        );
    }
}
