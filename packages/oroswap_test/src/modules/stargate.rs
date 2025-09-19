use anyhow::{Ok, Result as AnyResult};
use cosmwasm_schema::serde::de::DeserializeOwned;
use cosmwasm_std::{
    coins,
    testing::{MockApi, MockStorage},
    Addr, Api, BankMsg, Binary, BlockInfo, CustomMsg, CustomQuery, Empty, Querier, Storage,
    SubMsgResponse,
};

#[cfg(not(target_arch = "wasm32"))]
use cw_multi_test::{
    App, AppResponse, BankKeeper, BankSudo, CosmosRouter, DistributionKeeper, FailingModule,
    GovFailingModule, IbcFailingModule, Module, StakeKeeper, Stargate, StargateMsg, StargateQuery,
    SudoMsg, WasmKeeper,
};
use osmosis_std::types::osmosis::tokenfactory::v1beta1::MsgSetDenomMetadata;

use oroswap::token_factory::{
    MsgBurn, MsgCreateDenom, MsgCreateDenomResponse, MsgMint, MsgSetBeforeSendHook,
};

#[cfg(not(target_arch = "wasm32"))]
pub type StargateApp<ExecC = Empty, QueryC = Empty> = App<
    BankKeeper,
    MockApi,
    MockStorage,
    FailingModule<ExecC, QueryC, Empty>,
    WasmKeeper<ExecC, QueryC>,
    StakeKeeper,
    DistributionKeeper,
    IbcFailingModule,
    GovFailingModule,
    MockStargate,
>;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct MockStargate {}

#[cfg(not(target_arch = "wasm32"))]
impl Stargate for MockStargate {}

#[cfg(not(target_arch = "wasm32"))]
impl Module for MockStargate {
    type ExecT = StargateMsg;
    type QueryT = StargateQuery;
    type SudoT = Empty;

    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: Self::ExecT,
    ) -> AnyResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let StargateMsg {
            type_url, value, ..
        } = msg;

        match type_url.as_str() {
            MsgCreateDenom::TYPE_URL => {
                let tf_msg: MsgCreateDenom = value.try_into()?;
                #[cfg(not(any(feature = "injective", feature = "sei")))]
                let sender_address = tf_msg.creator.to_string();
                #[cfg(any(feature = "injective", feature = "sei"))]
                let sender_address = sender.to_string();
                let submsg_response = SubMsgResponse {
                    events: vec![],
                    data: Some(
                        MsgCreateDenomResponse {
                            creator: sender_address.clone(),
                            bank_admin: sender_address.clone(),
                            metadata_admin: sender_address.clone(),
                            denom: format!(
                                "coin.{}.{}",
                                sender_address, tf_msg.sub_denom
                            ),
                            minting_cap: "1000000000000000000000000000".to_string(),
                            can_change_minting_cap: false,
                            URI: "".to_string(),
                            URI_hash: "".to_string(),
                        }
                        .into(),
                    ),
                };
                Ok(submsg_response.into())
            }
            MsgMint::TYPE_URL => {
                let tf_msg: MsgMint = value.try_into()?;
                let mint_coins = tf_msg
                    .token
                    .expect("Empty token in tokenfactory MsgMint!");
                #[cfg(not(any(feature = "injective", feature = "sei")))]
                let to_address = tf_msg.recipient.to_string();
                #[cfg(any(feature = "injective", feature = "sei"))]
                let to_address = sender.to_string();
                let bank_sudo = BankSudo::Mint {
                    to_address,
                    amount: coins(mint_coins.amount.parse()?, mint_coins.denom),
                };
                router.sudo(api, storage, block, bank_sudo.into())
            }
            MsgBurn::TYPE_URL => {
                let tf_msg: MsgBurn = value.try_into()?;
                let burn_coins = tf_msg
                    .token
                    .expect("Empty token in tokenfactory MsgBurn!");
                let burn_msg = BankMsg::Burn {
                    amount: coins(burn_coins.amount.parse()?, burn_coins.denom),
                };
                router.execute(
                    api,
                    storage,
                    block,
                    Addr::unchecked(sender),
                    burn_msg.into(),
                )
            }
            MsgSetBeforeSendHook::TYPE_URL => {
                let before_hook_msg: MsgSetBeforeSendHook = value.try_into()?;
                let msg = BankSudo::SetHook {
                    contract_addr: before_hook_msg.cosmwasm_address,
                    denom: before_hook_msg.denom,
                };
                router.sudo(api, storage, block, SudoMsg::Bank(msg))
            }
            MsgSetDenomMetadata::TYPE_URL => {
                // TODO: Implement this if needed
                Ok(AppResponse::default())
            }
            _ => Err(anyhow::anyhow!(
                "Unexpected exec msg {type_url} from {sender:?}",
            )),
        }
    }
    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        _request: Self::QueryT,
    ) -> AnyResult<Binary> {
        Ok(Binary::default())
    }
    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _msg: Self::SudoT,
    ) -> AnyResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        unimplemented!("Sudo not implemented")
    }
}
