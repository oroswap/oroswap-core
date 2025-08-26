use cosmwasm_std::{Addr, Uint128, Decimal};
use std::str::FromStr;
use oroswap_core::asset::{Asset, AssetInfo};
use oroswap_core::factory::PairType;
use oroswap_pool_initializer::msg::{ExecuteMsg, InstantiateMsg, ProvideLiquidityParams};

/// Example: Instantiate the pool initializer contract
pub fn instantiate_pool_initializer(factory_addr: String) -> InstantiateMsg {
    InstantiateMsg {
        factory_addr,
        pair_creation_fee: Uint128::new(101000000), // 101 ZIG default fee
        fee_denom: "uzig".to_string(),
    }
}

/// Example: Initialize a XYK pool with native token liquidity
pub fn initialize_xyk_pool_native(
    pair_type: PairType,
    asset_infos: Vec<AssetInfo>,
    initial_liquidity: Vec<Asset>,
    slippage_tolerance: Option<Decimal>,
    receiver: Option<String>,
) -> ExecuteMsg {
    let liquidity_params = ProvideLiquidityParams {
        assets: initial_liquidity,
        slippage_tolerance,
        receiver,
        min_lp_to_receive: None,
    };
    
    ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type,
        asset_infos,
        init_params: None,
        liquidity: liquidity_params,
    }
}

/// Example: Initialize a stable pool with custom parameters
/// Note: This requires the pair_stable module to be available
pub fn initialize_stable_pool(
    asset_infos: Vec<AssetInfo>,
    initial_liquidity: Vec<Asset>,
    _amp: u64,
    slippage_tolerance: Option<Decimal>,
    receiver: Option<String>,
) -> ExecuteMsg {
    // For now, we'll use XYK as stable pools require additional modules
    let liquidity_params = ProvideLiquidityParams {
        assets: initial_liquidity,
        slippage_tolerance,
        receiver,
        min_lp_to_receive: None,
    };
    
    ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type: PairType::Xyk {},
        asset_infos,
        init_params: None,
        liquidity: liquidity_params,
    }
}

/// Example: Create a USDC/ZIG pool with initial liquidity
pub fn create_usdc_zig_pool_example() -> ExecuteMsg {
    let asset_infos = vec![
        AssetInfo::NativeToken {
            denom: "usdc".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "uzig".to_string(),
        },
    ];

    let initial_liquidity = vec![
        Asset {
            info: AssetInfo::NativeToken {
                denom: "usdc".to_string(),
            },
            amount: Uint128::new(1000000), // 1 USDC
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uzig".to_string(),
            },
            amount: Uint128::new(1000000), // 1 ZIG
        },
    ];

    let liquidity_params = ProvideLiquidityParams {
        assets: initial_liquidity,
        slippage_tolerance: Some(Decimal::from_str("0.01").unwrap()), // 1% slippage
        receiver: None, // LP tokens go to sender
        min_lp_to_receive: None,
    };

    ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type: PairType::Xyk {},
        asset_infos,
        init_params: None,
        liquidity: liquidity_params,
    }
}

/// Example: Create a CW20 token pool with native token
pub fn create_cw20_native_pool_example(
    cw20_contract: Addr,
    native_denom: String,
    cw20_amount: Uint128,
    native_amount: Uint128,
) -> ExecuteMsg {
    let asset_infos = vec![
        AssetInfo::Token {
            contract_addr: cw20_contract.clone(),
        },
        AssetInfo::NativeToken {
            denom: native_denom.clone(),
        },
    ];

    let initial_liquidity = vec![
        Asset {
            info: AssetInfo::Token {
                contract_addr: cw20_contract,
            },
            amount: cw20_amount,
        },
        Asset {
            info: AssetInfo::NativeToken {
                denom: native_denom,
            },
            amount: native_amount,
        },
    ];

    let liquidity_params = ProvideLiquidityParams {
        assets: initial_liquidity,
        slippage_tolerance: Some(Decimal::from_str("0.005").unwrap()), // 0.5% slippage
        receiver: None,
        min_lp_to_receive: None,
    };

    ExecuteMsg::CreatePairAndProvideLiquidity {
        pair_type: PairType::Xyk {},
        asset_infos,
        init_params: None,
        liquidity: liquidity_params,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instantiate_msg() {
        let factory_addr = "zig1factory1234567890abcdef".to_string();
        let msg = instantiate_pool_initializer(factory_addr.clone());
        assert_eq!(msg.factory_addr, factory_addr);
        assert_eq!(msg.pair_creation_fee, Uint128::new(101000000));
    }

    #[test]
    fn test_xyk_pool_creation() {
        let msg = create_usdc_zig_pool_example();
        match msg {
            ExecuteMsg::CreatePairAndProvideLiquidity {
                pair_type,
                asset_infos,
                init_params,
                liquidity,
            } => {
                assert_eq!(pair_type, PairType::Xyk {});
                assert_eq!(asset_infos.len(), 2);
                assert_eq!(liquidity.assets.len(), 2);
                assert_eq!(liquidity.slippage_tolerance, Some(Decimal::from_str("0.01").unwrap()));
                assert_eq!(liquidity.receiver, None);
                assert_eq!(init_params, None);
            }
            ExecuteMsg::UpdateConfig { .. } => {
                // This test doesn't cover UpdateConfig, so we'll skip it
                panic!("UpdateConfig not expected in this test");
            }
            ExecuteMsg::EmergencyRecovery {} => {
                // This test doesn't cover EmergencyRecovery, so we'll skip it
                panic!("EmergencyRecovery not expected in this test");
            }
        }
    }

    #[test]
    fn test_cw20_native_pool_creation() {
        let cw20_contract = Addr::unchecked("zig1token1234567890abcdef");
        let native_denom = "uzig".to_string();
        let cw20_amount = Uint128::new(1000000);
        let native_amount = Uint128::new(5000000);

        let msg = create_cw20_native_pool_example(
            cw20_contract.clone(),
            native_denom.clone(),
            cw20_amount,
            native_amount,
        );

        match msg {
            ExecuteMsg::CreatePairAndProvideLiquidity {
                pair_type,
                asset_infos,
                init_params,
                liquidity,
            } => {
                assert_eq!(pair_type, PairType::Xyk {});
                assert_eq!(asset_infos.len(), 2);
                assert_eq!(liquidity.assets.len(), 2);
                assert_eq!(liquidity.slippage_tolerance, Some(Decimal::from_str("0.005").unwrap()));
                assert_eq!(liquidity.receiver, None);
                assert_eq!(init_params, None);

                // Check asset infos
                if let AssetInfo::Token { contract_addr } = &asset_infos[0] {
                    assert_eq!(contract_addr, &cw20_contract);
                } else {
                    panic!("Expected token asset info");
                }

                if let AssetInfo::NativeToken { denom } = &asset_infos[1] {
                    assert_eq!(denom, &native_denom);
                } else {
                    panic!("Expected native token asset info");
                }
            }
            ExecuteMsg::UpdateConfig { .. } => {
                // This test doesn't cover UpdateConfig, so we'll skip it
                panic!("UpdateConfig not expected in this test");
            }
            ExecuteMsg::EmergencyRecovery {} => {
                // This test doesn't cover EmergencyRecovery, so we'll skip it
                panic!("EmergencyRecovery not expected in this test");
            }
        }
    }
}

fn main() {
    println!("Pool Initializer Usage Examples");
    println!("This file contains example functions for using the pool initializer contract.");
    println!("Run the tests to see the examples in action:");
    println!("cargo test --package oroswap-pool-initializer --example usage");
}
