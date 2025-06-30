use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Deps, Order, StdResult};
use cw_storage_plus::{Bound, Item, Map};
use itertools::Itertools;
use oroswap::asset::AssetInfo;
use oroswap::factory::{Config, PairConfig, TrackerConfig, PairType, StartAfter};
use oroswap::common::OwnershipProposal;


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

/// Saves created pairs (from olders to latest)
pub const PAIRS: Map<&[u8], Addr> = Map::new("pair_info");

/// Track config for tracking contract
pub const TRACKER_CONFIG: Item<TrackerConfig> = Item::new("tracker_config");

/// Calculates a pair key from the specified parameters in the `asset_infos` variable.
///
/// `asset_infos` is an array with multiple items of type [`AssetInfo`].
pub fn pair_key(asset_infos: &[AssetInfo], pair_type: &PairType) -> Vec<u8> {
    let mut key = asset_infos
        .iter()
        .map(AssetInfo::as_bytes)
        .sorted()
        .flatten()
        .copied()
        .collect::<Vec<u8>>();
    
    // Append pair type to the key
    key.extend_from_slice(pair_type.to_string().as_bytes());
    key
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
    if !asset_infos.iter().all_unique() {
        return Err(ContractError::DoublingAssets {});
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

        // Verify key format includes pair type
        let mut expected_key = asset_infos
            .iter()
            .map(AssetInfo::as_bytes)
            .sorted()
            .flatten()
            .copied()
            .collect::<Vec<u8>>();
        expected_key.extend_from_slice(b"xyk");
        assert_eq!(xyk_key, expected_key);
    }

    #[test]
    fn test_legacy_start_after() {
        fn legacy_calc_range_start(start_after: Option<[AssetInfo; 2]>) -> Option<Vec<u8>> {
            start_after.map(|asset_infos| {
                let mut asset_infos = asset_infos.to_vec();
                asset_infos.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

                let mut v = [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()]
                    .concat()
                    .as_slice()
                    .to_vec();
                v.extend_from_slice(b"xyk"); // Add pair type
                v.push(1);
                v
            })
        }

        for asset_infos in get_test_case() {
            assert_eq!(
                legacy_calc_range_start(Some(asset_infos.clone())),
                calc_range_start(Some(StartAfter {
                    asset_infos: asset_infos.to_vec(),
                    pair_type: PairType::Xyk {},
                }))
            );
        }
    }
}
