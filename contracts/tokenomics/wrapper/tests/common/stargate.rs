use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use anyhow::Result as AnyResult;
use cosmwasm_schema::schemars::JsonSchema;
use cosmwasm_schema::serde::de::DeserializeOwned;
use cosmwasm_std::{
    coin, Addr, Api, Binary, BlockInfo, CustomQuery, Querier, Storage, SubMsgResponse,
};
use cw_multi_test::{AppResponse, BankSudo, CosmosRouter, Stargate};
use oroswap::token_factory::{
    MsgBurn, MsgCreateDenom, MsgCreateDenomResponse, MsgMint, MsgProposeDenomAdmin,
    MsgSetBeforeSendHook, MsgSetDenomMetadata,
};

/// Decoded tokenfactory messages the contract emitted during a test, captured
/// so tests can assert the payload (not just that the call succeeded).
///
/// Mocked execute always returns success, so without this buffer we couldn't
/// distinguish a correct patch from one that silently set every Metadata
/// field to the wrong value.
#[derive(Default, Debug)]
pub struct CapturedTxs {
    pub set_metadata: Vec<MsgSetDenomMetadata>,
    pub propose_admin: Vec<MsgProposeDenomAdmin>,
}

/// A mock `StargateKeeper` that handles ZigChain tokenfactory messages during
/// integration tests. Mirrors the equivalent in the staking contract's tests.
///
/// Captures decoded `MsgSetDenomMetadata` and `MsgProposeDenomAdmin` payloads
/// in a shared `Arc<Mutex<_>>` so tests holding a clone of the handle can
/// assert on the exact bytes the contract sent to the chain.
#[derive(Default, Clone)]
pub struct StargateKeeper {
    pub captured: Arc<Mutex<CapturedTxs>>,
}

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
            // ZigChain factory: create denom
            MsgCreateDenom::TYPE_URL => {
                let tf_msg: MsgCreateDenom = value.try_into()?;
                let new_denom = format!("coin.{}.{}", tf_msg.creator, tf_msg.sub_denom);
                let submsg_response = SubMsgResponse {
                    events: vec![],
                    data: Some(
                        MsgCreateDenomResponse {
                            denom: new_denom,
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
            // ZigChain factory: mint tokens
            MsgMint::TYPE_URL => {
                let tf_msg: MsgMint = value.try_into()?;
                let mint_coins = tf_msg
                    .token
                    .expect("Empty amount in tokenfactory MsgMint!");
                let cw_coin = coin(mint_coins.amount.parse()?, mint_coins.denom);
                let bank_sudo = BankSudo::Mint {
                    to_address: tf_msg.recipient.clone(),
                    amount: vec![cw_coin],
                };
                router.sudo(api, storage, block, bank_sudo.into())
            }
            // ZigChain factory: burn tokens
            MsgBurn::TYPE_URL => {
                let tf_msg: MsgBurn = value.try_into()?;
                let burn_coins = tf_msg
                    .token
                    .expect("Empty amount in tokenfactory MsgBurn!");
                let cw_coin = coin(burn_coins.amount.parse()?, burn_coins.denom);
                let burn_msg = cosmwasm_std::BankMsg::Burn {
                    amount: vec![cw_coin],
                };
                router.execute(
                    api,
                    storage,
                    block,
                    Addr::unchecked(&tf_msg.signer),
                    burn_msg.into(),
                )
            }
            // Set before-send hook
            MsgSetBeforeSendHook::TYPE_URL => {
                let tf_msg: MsgSetBeforeSendHook = value.try_into()?;
                let bank_sudo = BankSudo::SetHook {
                    denom: tf_msg.denom,
                    contract_addr: tf_msg.cosmwasm_address,
                };
                router.sudo(api, storage, block, bank_sudo.into())
            }
            // Set denom metadata — capture for assertion, then return success
            MsgSetDenomMetadata::TYPE_URL => {
                let tf_msg: MsgSetDenomMetadata = value.try_into()?;
                self.captured
                    .lock()
                    .expect("captured mutex poisoned")
                    .set_metadata
                    .push(tf_msg);
                Ok(AppResponse::default())
            }
            // Propose denom admin — capture for assertion, then return success
            MsgProposeDenomAdmin::TYPE_URL => {
                let tf_msg: MsgProposeDenomAdmin = value.try_into()?;
                self.captured
                    .lock()
                    .expect("captured mutex poisoned")
                    .propose_admin
                    .push(tf_msg);
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
