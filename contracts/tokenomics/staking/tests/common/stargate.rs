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
                            new_token_denom: format!(
                                "coin.{}.{}",
                                tf_msg.sender, tf_msg.subdenom
                            ),
                        }
                        .into(),
                    ),
                };
                Ok(submsg_response.into())
            }
            MsgMint::TYPE_URL => {
                let tf_msg: MsgMint = value.try_into()?;
                let mint_coins = tf_msg
                    .amount
                    .expect("Empty amount in tokenfactory MsgMint!");
                let cw_coin = coin(mint_coins.amount.parse()?, mint_coins.denom);
                let bank_sudo = BankSudo::Mint {
                    to_address: tf_msg.mint_to_address.clone(),
                    amount: vec![cw_coin],
                };
                router.sudo(api, storage, block, bank_sudo.into())
            }
            MsgBurn::TYPE_URL => {
                let tf_msg: MsgBurn = value.try_into()?;
                let burn_coins = tf_msg
                    .amount
                    .expect("Empty amount in tokenfactory MsgBurn!");
                let cw_coin = coin(burn_coins.amount.parse()?, burn_coins.denom);
                let burn_msg = BankMsg::Burn {
                    amount: vec![cw_coin],
                };
                // burn_from_address is empty in non-zigchain; fall back to sender
                let burn_from = if tf_msg.burn_from_address.is_empty() {
                    tf_msg.sender.clone()
                } else {
                    tf_msg.burn_from_address.clone()
                };
                router.execute(
                    api,
                    storage,
                    block,
                    Addr::unchecked(&burn_from),
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
            _ => {
                // Silently accept unknown stargate messages (e.g. MsgSetDenomMetadata)
                let _ = sender;
                Ok(AppResponse::default())
            }
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
