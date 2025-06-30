use cosmwasm_schema::cw_serde;
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, Coin, ContractResult, Empty, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use std::collections::HashMap;

use oroswap::asset::{Asset, AssetInfo, PairInfo};
use oroswap::factory::PairType;
use oroswap::pair::SimulationResponse;
use cw20::{BalanceResponse, Cw20QueryMsg, TokenInfoResponse};

#[cw_serde]
pub enum QueryMsg {
    Pair {
        asset_infos: Vec<AssetInfo>,
        pair_type: PairType,
    },
    Simulation {
        offer_asset: Asset,
        ask_asset_info: Option<AssetInfo>,
    },
}

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies.
/// This uses the Oroswap CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: Default::default(),
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    token_querier: TokenQuerier,
    oroswap_factory_querier: OroswapFactoryQuerier,
}

#[derive(Clone, Default)]
pub struct TokenQuerier {
    // This lets us iterate over all pairs that match the first string
    balances: HashMap<String, HashMap<String, Uint128>>,
}

impl TokenQuerier {
    pub fn new(balances: &[(&String, &[(&String, &Uint128)])]) -> Self {
        TokenQuerier {
            balances: balances_to_map(balances),
        }
    }
}

pub(crate) fn balances_to_map(
    balances: &[(&String, &[(&String, &Uint128)])],
) -> HashMap<String, HashMap<String, Uint128>> {
    let mut balances_map: HashMap<String, HashMap<String, Uint128>> = HashMap::new();
    for (contract_addr, balances) in balances.iter() {
        let mut contract_balances_map: HashMap<String, Uint128> = HashMap::new();
        for (addr, balance) in balances.iter() {
            contract_balances_map.insert(addr.to_string(), **balance);
        }

        balances_map.insert(contract_addr.to_string(), contract_balances_map);
    }
    balances_map
}

#[derive(Clone, Default)]
pub struct OroswapFactoryQuerier {
    pairs: HashMap<String, (String, PairType)>,  // Store pair address and type
}

impl OroswapFactoryQuerier {
    pub fn new(pairs: &[(&String, &String, PairType)]) -> Self {
        OroswapFactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &String, PairType)]) -> HashMap<String, (String, PairType)> {
    let mut pairs_map: HashMap<String, (String, PairType)> = HashMap::new();
    for (key, pair, pair_type) in pairs.iter() {
        pairs_map.insert(key.to_string(), (pair.to_string(), pair_type.clone()));
    }
    pairs_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

#[cw_serde]
pub enum MockQueryMsg {
    Price {},
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                if contract_addr.to_string().starts_with("token")
                    || contract_addr.to_string().starts_with("asset")
                {
                    self.handle_cw20(&contract_addr, &msg)
                } else {
                    self.handle_default(&msg)
                }
            }
            _ => self.base.handle_query(request),
        }
    }

    fn handle_default(&self, msg: &Binary) -> QuerierResult {
        match from_json(&msg).unwrap() {
            QueryMsg::Pair { asset_infos, pair_type } => {
                let key = asset_infos[0].to_string() + asset_infos[1].to_string().as_str();
                match self.oroswap_factory_querier.pairs.get(&key) {
                    Some((pair_addr, stored_pair_type)) => {
                        if *stored_pair_type == pair_type {
                            SystemResult::Ok(ContractResult::from(to_json_binary(&PairInfo {
                                contract_addr: Addr::unchecked(pair_addr),
                                liquidity_token: "liquidity".to_string(),
                                asset_infos: asset_infos.clone(),
                                pair_type: pair_type.clone(),
                            })))
                        } else {
                            SystemResult::Err(SystemError::InvalidRequest {
                                error: "Pair type mismatch".to_string(),
                                request: msg.as_slice().into(),
                            })
                        }
                    }
                    None => SystemResult::Err(SystemError::InvalidRequest {
                        error: "No pair info exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                }
            }
            QueryMsg::Simulation { offer_asset, .. } => {
                SystemResult::Ok(ContractResult::from(to_json_binary(&SimulationResponse {
                    return_amount: offer_asset.amount,
                    commission_amount: Uint128::zero(),
                    spread_amount: Uint128::zero(),
                })))
            }
        }
    }

    fn handle_cw20(&self, contract_addr: &String, msg: &Binary) -> QuerierResult {
        match from_json(&msg).unwrap() {
            Cw20QueryMsg::TokenInfo {} => {
                let balances: &HashMap<String, Uint128> =
                    match self.token_querier.balances.get(contract_addr) {
                        Some(balances) => balances,
                        None => {
                            return SystemResult::Err(SystemError::Unknown {});
                        }
                    };

                let mut total_supply = Uint128::zero();

                for balance in balances {
                    total_supply += *balance.1;
                }

                SystemResult::Ok(ContractResult::from(to_json_binary(&TokenInfoResponse {
                    name: "mAPPL".to_string(),
                    symbol: "mAPPL".to_string(),
                    decimals: 6,
                    total_supply: total_supply,
                })))
            }
            Cw20QueryMsg::Balance { address } => {
                let balances: &HashMap<String, Uint128> =
                    match self.token_querier.balances.get(contract_addr) {
                        Some(balances) => balances,
                        None => {
                            return SystemResult::Err(SystemError::Unknown {});
                        }
                    };

                let balance = match balances.get(&address) {
                    Some(v) => v,
                    None => {
                        return SystemResult::Err(SystemError::Unknown {});
                    }
                };

                SystemResult::Ok(ContractResult::from(to_json_binary(&BalanceResponse {
                    balance: *balance,
                })))
            }
            _ => panic!("DO NOT ENTER HERE"),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            token_querier: TokenQuerier::default(),
            oroswap_factory_querier: OroswapFactoryQuerier::default(),
        }
    }

    pub fn with_balance(&mut self, balances: &[(&String, &[Coin])]) {
        for (addr, balance) in balances {
            self.base.update_balance(addr.as_str(), balance.to_vec());
        }
    }

    pub fn with_token_balances(&mut self, balances: &[(&String, &[(&String, &Uint128)])]) {
        self.token_querier = TokenQuerier::new(balances);
    }

    pub fn with_oroswap_pairs(&mut self, pairs: &[(&String, &String, PairType)]) {
        self.oroswap_factory_querier = OroswapFactoryQuerier::new(pairs);
    }
}
