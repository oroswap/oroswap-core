use std::fmt::Debug;

use anyhow::Result as AnyResult;
use cosmwasm_schema::schemars::JsonSchema;
use cosmwasm_schema::serde::de::DeserializeOwned;
use cosmwasm_std::{
    coin, Addr, Api, BankMsg, Binary, BlockInfo, CustomQuery, Querier, Storage, SubMsgResponse,
};
use cw_multi_test::{AppResponse, BankSudo, CosmosRouter, Stargate};
use oroswap::token_factory::{
    MsgBurn, MsgCreateDenom, MsgCreateDenomResponse, MsgMint, MsgSetBeforeSendHook,
    MsgSetDenomMetadata,
};

#[derive(Default)]
pub struct StargateKeeper {}

impl Stargate for StargateKeeper {
    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        type_url: String,
        value: Binary,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        match type_url.as_str() {
            MsgCreateDenom::TYPE_URL => {
                let tf_msg: MsgCreateDenom = value.try_into()?;
                let submsg_response = SubMsgResponse {
                    events: vec![],
                    data: Some(
                        MsgCreateDenomResponse {
                            denom: format!("coin.{}.{}", tf_msg.creator, tf_msg.sub_denom),
                            creator: tf_msg.creator,
                            bank_admin: "".to_string(),
                            metadata_admin: "".to_string(),
                            minting_cap: tf_msg.minting_cap,
                            can_change_minting_cap: tf_msg.can_change_minting_cap,
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
                    .expect("Empty amount in tokenfactory MsgMint!");
                let cw_coin = coin(mint_coins.amount.parse()?, mint_coins.denom);
                let bank_sudo = BankSudo::Mint {
                    to_address: tf_msg.recipient.clone(),
                    amount: vec![cw_coin.clone()],
                };

                router.sudo(api, storage, block, bank_sudo.into())
            }
            MsgBurn::TYPE_URL => {
                let tf_msg: MsgBurn = value.try_into()?;
                let burn_coins = tf_msg
                    .token
                    .expect("Empty amount in tokenfactory MsgBurn!");
                let cw_coin = coin(burn_coins.amount.parse()?, burn_coins.denom);
                let burn_msg = BankMsg::Burn {
                    amount: vec![cw_coin.clone()],
                };

                router.execute(
                    api,
                    storage,
                    block,
                    Addr::unchecked(&tf_msg.signer),
                    burn_msg.into(),
                )
            }
            MsgSetBeforeSendHook::TYPE_URL => {
                let tf_msg: MsgSetBeforeSendHook = value.try_into()?;

                let bank_sudo = BankSudo::SetHook {
                    denom: tf_msg.denom,
                    contract_addr: tf_msg.cosmwasm_address,
                };

                router.sudo(api, storage, block, bank_sudo.into())
            }
            MsgSetDenomMetadata::TYPE_URL => {
                // Just return success for metadata
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
        _path: String,
        _data: Binary,
    ) -> AnyResult<Binary> {
        unimplemented!("Stargate queries are not implemented")
    }
}
