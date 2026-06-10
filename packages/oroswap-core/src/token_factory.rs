pub use cosmos_sdk_proto::cosmos::base::v1beta1::Coin as ProtoCoin;
use cosmwasm_std::{Binary, Coin, CosmosMsg, CustomMsg, StdError, Uint128};

#[cfg(any(feature = "zigchain"))]
use cosmwasm_std::BankMsg;

use prost::Message;

use crate::common::LP_MAX_SUPPLY;

#[cfg(not(any(feature = "zigchain")))]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgCreateDenomResponse {
    #[prost(string, tag = "1")]
    pub new_token_denom: ::prost::alloc::string::String,
}

/*
type MsgCreateDenomResponse struct {
    Creator            string                 `protobuf:"bytes,1,opt,name=creator,proto3" json:"creator,omitempty"`
    BankAdmin          string                 `protobuf:"bytes,2,opt,name=bankAdmin,proto3" json:"bankAdmin,omitempty"`
    MetadataAdmin      string                 `protobuf:"bytes,3,opt,name=metadataAdmin,proto3" json:"metadataAdmin,omitempty"`
    Denom              string                 `protobuf:"bytes,4,opt,name=denom,proto3" json:"denom,omitempty"`
    MaxSupply          cosmossdk_io_math.Uint `protobuf:"bytes,5,opt,name=maxSupply,proto3,customtype=cosmossdk.io/math.Uint" json:"maxSupply"`
    CanChangeMaxSupply bool                   `protobuf:"varint,6,opt,name=canChangeMaxSupply,proto3" json:"canChangeMaxSupply,omitempty"`
    URI                string                 `protobuf:"bytes,7,opt,name=URI,proto3" json:"URI,omitempty"`
    URIHash            string                 `protobuf:"bytes,8,opt,name=URIHash,proto3" json:"URIHash,omitempty"`
}
*/
#[cfg(feature = "zigchain")]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgCreateDenomResponse {
    #[prost(string, tag = "1")]
    pub creator: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub bank_admin: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub metadata_admin: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub denom: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub minting_cap: ::prost::alloc::string::String,
    #[prost(bool, tag = "6")]
    pub can_change_minting_cap: bool,
    #[prost(string, tag = "7")]
    pub URI: ::prost::alloc::string::String,
    #[prost(string, tag = "8")]
    pub URI_hash: ::prost::alloc::string::String,
}

impl MsgCreateDenomResponse {
    pub fn to_proto_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode(&mut buf).unwrap();
        buf
    }
}

impl From<MsgCreateDenomResponse> for Binary {
    fn from(msg: MsgCreateDenomResponse) -> Self {
        Binary(msg.to_proto_bytes())
    }
}

impl TryFrom<Binary> for MsgCreateDenomResponse {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(
                format!(
                    "MsgCreateDenomResponse Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                    binary,
                    binary.to_vec(),
                    e
                ),
            )
        })
    }
}

#[cfg(not(any(feature = "zigchain")))]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgCreateDenom {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    /// subdenom can be up to 44 "alphanumeric" characters long.
    #[prost(string, tag = "2")]
    pub subdenom: ::prost::alloc::string::String,
}

// #[cfg(feature = "sei")]
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct MsgCreateDenom {
//     /// subdenom can be up to 44 "alphanumeric" characters long.
//     #[prost(string, tag = "2")]
//     pub subdenom: ::prost::alloc::string::String,
// }

#[cfg(feature = "zigchain")]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgCreateDenom {
    #[prost(string, tag = "1")]
    pub creator: ::prost::alloc::string::String,
    /// sub_denom can be up to 44 "alphanumeric" characters long.
    #[prost(string, tag = "2")]
    pub sub_denom: ::prost::alloc::string::String,
    /// minting_cap is the maximum supply of the token that can be minted. set it to be 1 by default.
    #[prost(string, tag = "3")]
    pub minting_cap: ::prost::alloc::string::String,
    /// can_change_minting_cap is the mintable status of the token. set it to be true by default.
    #[prost(bool, tag = "4")]
    pub can_change_minting_cap: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgSetDenomMetadata {
    #[prost(string, tag = "1")]
    pub signer: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub metadata: ::core::option::Option<Metadata>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Metadata {
    #[prost(string, tag = "1")]
    pub description: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "2")]
    pub denom_units: ::prost::alloc::vec::Vec<DenomUnit>,
    #[prost(string, tag = "3")]
    pub base: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub display: ::prost::alloc::string::String,
    #[prost(string, tag = "5")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "6")]
    pub symbol: ::prost::alloc::string::String,
    #[prost(string, tag = "7")]
    pub uri: ::prost::alloc::string::String,
    #[prost(string, tag = "8")]
    pub uri_hash: ::prost::alloc::string::String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DenomUnit {
    #[prost(string, tag = "1")]
    pub denom: ::prost::alloc::string::String,
    #[prost(uint32, tag = "2")]
    pub exponent: u32,
    #[prost(string, repeated, tag = "3")]
    pub aliases: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}

impl MsgSetDenomMetadata {
    #[cfg(feature = "zigchain")]
    pub const TYPE_URL: &'static str = "/zigchain.factory.MsgSetDenomMetadata";
}

impl TryFrom<Binary> for MsgSetDenomMetadata {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(format!(
                "MsgSetDenomMetadata Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                binary,
                binary.to_vec(),
                e
            ))
        })
    }
}

impl MsgCreateDenom {
    #[cfg(not(any(feature = "zigchain")))]
    pub const TYPE_URL: &'static str = "/osmosis.tokenfactory.v1beta1.MsgCreateDenom";
    #[cfg(feature = "zigchain")]
    pub const TYPE_URL: &'static str = "/zigchain.factory.MsgCreateDenom";
    // #[cfg(feature = "sei")]
    // pub const TYPE_URL: &'static str = "/seiprotocol.seichain.tokenfactory.v1beta1.MsgCreateDenom";
}

impl TryFrom<Binary> for MsgCreateDenom {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(format!(
                "MsgCreateDenom Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                binary,
                binary.to_vec(),
                e
            ))
        })
    }
}

#[cfg(not(any(feature = "zigchain")))]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgBurn {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub amount: ::core::option::Option<cosmos_sdk_proto::cosmos::base::v1beta1::Coin>,
    #[prost(string, tag = "3")]
    pub burn_from_address: ::prost::alloc::string::String,
}

#[cfg(feature = "zigchain")]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgBurn {
    #[prost(string, tag = "1")]
    pub signer: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub token: ::core::option::Option<cosmos_sdk_proto::cosmos::base::v1beta1::Coin>,
}

// #[cfg(feature = "sei")]
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct MsgBurn {
//     #[prost(message, optional, tag = "2")]
//     pub amount: ::core::option::Option<cosmos_sdk_proto::cosmos::base::v1beta1::Coin>,
// }

impl MsgBurn {
    #[cfg(not(any(feature = "zigchain")))]
    pub const TYPE_URL: &'static str = "/osmosis.tokenfactory.v1beta1.MsgBurn";
    #[cfg(feature = "zigchain")]
    pub const TYPE_URL: &'static str = "/zigchain.factory.MsgBurnTokens";
    // #[cfg(feature = "sei")]
    // pub const TYPE_URL: &'static str = "/seiprotocol.seichain.tokenfactory.v1beta1.MsgBurn";
}

impl TryFrom<Binary> for MsgBurn {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(format!(
                "MsgBurn Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                binary,
                binary.to_vec(),
                e
            ))
        })
    }
}

#[cfg(not(any(feature = "zigchain")))]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgMint {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub amount: ::core::option::Option<cosmos_sdk_proto::cosmos::base::v1beta1::Coin>,
    #[prost(string, tag = "3")]
    pub mint_to_address: ::prost::alloc::string::String,
}

#[cfg(feature = "zigchain")]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgMint {
    #[prost(string, tag = "1")]
    pub signer: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub token: ::core::option::Option<cosmos_sdk_proto::cosmos::base::v1beta1::Coin>,
    #[prost(string, tag = "3")]
    pub recipient: ::prost::alloc::string::String,
}

// #[cfg(feature = "sei")]
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct MsgMint {
//     #[prost(message, optional, tag = "2")]
//     pub amount: ::core::option::Option<cosmos_sdk_proto::cosmos::base::v1beta1::Coin>,
// }

impl MsgMint {
    #[cfg(not(any(feature = "zigchain")))]
    pub const TYPE_URL: &'static str = "/osmosis.tokenfactory.v1beta1.MsgMint";
    #[cfg(feature = "zigchain")]
    pub const TYPE_URL: &'static str = "/zigchain.factory.MsgMintAndSendTokens";
    // #[cfg(feature = "sei")]
    // pub const TYPE_URL: &'static str = "/seiprotocol.seichain.tokenfactory.v1beta1.MsgMint";
}

impl TryFrom<Binary> for MsgMint {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(format!(
                "MsgMint Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                binary,
                binary.to_vec(),
                e
            ))
        })
    }
}

#[cfg(not(any(feature = "zigchain")))]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgSetBeforeSendHook {
    #[prost(string, tag = "1")]
    pub sender: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub denom: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub cosmwasm_address: ::prost::alloc::string::String,
}

#[cfg(feature = "zigchain")]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgSetBeforeSendHook {
    #[prost(string, tag = "1")]
    pub signer: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub denom: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub cosmwasm_address: ::prost::alloc::string::String,
}

impl MsgSetBeforeSendHook {
    #[cfg(not(any(feature = "zigchain")))]
    pub const TYPE_URL: &'static str = "/osmosis.tokenfactory.v1beta1.MsgSetBeforeSendHook";
    #[cfg(feature = "zigchain")]
    pub const TYPE_URL: &'static str = "/zigchain.factory.MsgSetBeforeSendHook";
}

impl TryFrom<Binary> for MsgSetBeforeSendHook {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(format!(
                "MsgSetBeforeSendHook Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                binary,
                binary.to_vec(),
                e
            ))
        })
    }
}

pub fn tf_create_denom_msg<T>(sender: impl Into<String>, denom: impl Into<String>) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    #[cfg(not(any(feature = "zigchain")))]
    let create_denom_msg = MsgCreateDenom {
        sender: sender.into(),
        subdenom: denom.into(),
    };

    #[cfg(feature = "zigchain")]
    let create_denom_msg = MsgCreateDenom {
        sub_denom: denom.into(),
        creator: sender.into(),
        minting_cap: LP_MAX_SUPPLY.to_string(),
        can_change_minting_cap: false,
    };

    CosmosMsg::Stargate {
        type_url: MsgCreateDenom::TYPE_URL.to_string(),
        value: Binary::from(create_denom_msg.encode_to_vec()),
    }
}

/// Same as `tf_create_denom_msg` but with an explicit, caller-supplied
/// `minting_cap` instead of the generic `LP_MAX_SUPPLY` constant.
///
/// FIND-005 (audit follow-up): contracts that wrap a known finite source
/// asset (e.g. the wrapper contract wrapping a bridged ERC-20) should
/// configure the tokenfactory cap to match that asset's actual maximum
/// supply rather than `u64::MAX`. This pins the wrapped denom's supply
/// assumption on-chain and removes reliance on a shared constant that
/// was correct for pair-LP tokens but loose for bridged asset wrappers.
///
/// `can_change_minting_cap` is intentionally `false` — the cap is meant
/// to be a load-bearing invariant set once at instantiate.
///
/// The non-zigchain `MsgCreateDenom` variant has no cap field, so the
/// supplied value is silently ignored on chains that don't support it.
pub fn tf_create_denom_msg_with_cap<T>(
    sender: impl Into<String>,
    denom: impl Into<String>,
    minting_cap: Uint128,
) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    #[cfg(not(any(feature = "zigchain")))]
    let create_denom_msg = {
        let _ = minting_cap; // not part of the non-zigchain proto shape
        MsgCreateDenom {
            sender: sender.into(),
            subdenom: denom.into(),
        }
    };

    #[cfg(feature = "zigchain")]
    let create_denom_msg = MsgCreateDenom {
        sub_denom: denom.into(),
        creator: sender.into(),
        minting_cap: minting_cap.to_string(),
        can_change_minting_cap: false,
    };

    CosmosMsg::Stargate {
        type_url: MsgCreateDenom::TYPE_URL.to_string(),
        value: Binary::from(create_denom_msg.encode_to_vec()),
    }
}

pub fn tf_mint_msg<T>(
    sender: impl Into<String>,
    coin: Coin,
    receiver: impl Into<String>,
) -> Vec<CosmosMsg<T>>
where
    T: CustomMsg,
{
    let sender_addr: String = sender.into();
    let receiver_addr: String = receiver.into();

    #[cfg(not(any(feature = "zigchain")))]
    let mint_msg = MsgMint {
        sender: sender_addr.clone(),
        amount: Some(ProtoCoin {
            denom: coin.denom.to_string(),
            amount: coin.amount.to_string(),
        }),
        mint_to_address: receiver_addr.clone(),
    };

    #[cfg(feature = "zigchain")]
    let mint_msg = MsgMint {
        signer: sender_addr.clone(),
        token: Some(ProtoCoin {
            denom: coin.denom.to_string(),
            amount: coin.amount.to_string(),
        }),
        recipient: receiver_addr.clone(),
    };

    // #[cfg(feature = "sei")]
    // let mint_msg = MsgMint {
    //     amount: Some(ProtoCoin {
    //         denom: coin.denom.to_string(),
    //         amount: coin.amount.to_string(),
    //     }),
    // };

    // #[cfg(not(any(feature = "zigchain")))]
    return vec![CosmosMsg::Stargate {
        type_url: MsgMint::TYPE_URL.to_string(),
        value: Binary::from(mint_msg.encode_to_vec()),
    }];

    // #[cfg(any(feature = "injective", feature = "sei"))]
    // if sender_addr == receiver_addr {
    //     vec![CosmosMsg::Stargate {
    //         type_url: MsgMint::TYPE_URL.to_string(),
    //         value: Binary::from(mint_msg.encode_to_vec()),
    //     }]
    // } else {
    //     vec![
    //         CosmosMsg::Stargate {
    //             type_url: MsgMint::TYPE_URL.to_string(),
    //             value: Binary::from(mint_msg.encode_to_vec()),
    //         },
    //         BankMsg::Send {
    //             to_address: receiver_addr,
    //             amount: vec![coin],
    //         }
    //         .into(),
    //     ]
    // }
}

pub fn tf_burn_msg<T>(sender: impl Into<String>, coin: Coin) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    #[cfg(not(any(feature = "zigchain")))]
    let burn_msg = MsgBurn {
        sender: sender.into(),
        amount: Some(ProtoCoin {
            denom: coin.denom,
            amount: coin.amount.to_string(),
        }),
        burn_from_address: "".to_string(),
    };

    #[cfg(feature = "zigchain")]
    let burn_msg = MsgBurn {
        signer: sender.into(),
        token: Some(ProtoCoin {
            denom: coin.denom,
            amount: coin.amount.to_string(),
        }),
    };

    // #[cfg(feature = "sei")]
    // let burn_msg = MsgBurn {
    //     amount: Some(ProtoCoin {
    //         denom: coin.denom,
    //         amount: coin.amount.to_string(),
    //     }),
    // };

    CosmosMsg::Stargate {
        type_url: MsgBurn::TYPE_URL.to_string(),
        value: Binary::from(burn_msg.encode_to_vec()),
    }
}

pub fn tf_before_send_hook_msg<T>(
    sender: impl Into<String>,
    denom: impl Into<String>,
    cosmwasm_address: impl Into<String>,
) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    #[cfg(not(any(feature = "zigchain")))]
    let msg = MsgSetBeforeSendHook {
        sender: sender.into(),
        denom: denom.into(),
        cosmwasm_address: cosmwasm_address.into(),
    };

    #[cfg(feature = "zigchain")]
    let msg = MsgSetBeforeSendHook {
        signer: sender.into(),
        denom: denom.into(),
        cosmwasm_address: cosmwasm_address.into(),
    };

    CosmosMsg::Stargate {
        type_url: MsgSetBeforeSendHook::TYPE_URL.to_string(),
        value: Binary::from(msg.encode_to_vec()),
    }
}

#[cfg(feature = "zigchain")]
pub fn tf_set_denom_metadata_msg<T>(
    sender: impl Into<String>,
    denom: String,
    symbol: String,
    decimals: u32,
    uri: Option<String>,
) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    let metadata_uri = uri.unwrap_or_default();
    let msg = MsgSetDenomMetadata {
        signer: sender.into(),
        metadata: Some(Metadata {
            description: format!("Oroswap {} Token", symbol),
            denom_units: vec![
                DenomUnit {
                    denom: denom.clone(),
                    exponent: 0,
                    aliases: vec![],
                },
                DenomUnit {
                    denom: symbol.clone().to_lowercase(),
                    exponent: decimals,
                    aliases: vec![],
                },
            ],
            base: denom.clone(),
            display: symbol.to_lowercase(),
            name: format!("Oroswap {}", symbol),
            symbol: symbol,
            uri: metadata_uri,
            uri_hash: "".to_string(), // Optional or handled by chain if uri provided
        }),
    };

    CosmosMsg::Stargate {
        type_url: MsgSetDenomMetadata::TYPE_URL.to_string(),
        value: Binary::from(msg.encode_to_vec()),
    }
}

/// Build a ZigChain factory `MsgSetDenomMetadata` with explicit `name`, `symbol`,
/// `description` and `display`. Falls back to the legacy "Oroswap {symbol}" naming
/// when a caller doesn't supply overrides.
#[cfg(feature = "zigchain")]
#[allow(clippy::too_many_arguments)]
pub fn tf_set_denom_metadata_full_msg<T>(
    sender: impl Into<String>,
    denom: String,
    name: String,
    symbol: String,
    description: String,
    display: String,
    decimals: u32,
    uri: Option<String>,
    uri_hash: Option<String>,
) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    let metadata_uri = uri.unwrap_or_default();
    let metadata_uri_hash = uri_hash.unwrap_or_default();
    let base_alias = format!("u{}", display);
    let msg = MsgSetDenomMetadata {
        signer: sender.into(),
        metadata: Some(Metadata {
            description,
            denom_units: vec![
                DenomUnit {
                    denom: denom.clone(),
                    exponent: 0,
                    aliases: vec![base_alias],
                },
                DenomUnit {
                    denom: display.clone(),
                    exponent: decimals,
                    aliases: vec![],
                },
            ],
            base: denom,
            display,
            name,
            symbol,
            uri: metadata_uri,
            uri_hash: metadata_uri_hash,
        }),
    };

    CosmosMsg::Stargate {
        type_url: MsgSetDenomMetadata::TYPE_URL.to_string(),
        value: Binary::from(msg.encode_to_vec()),
    }
}

// ---------------------------------------------------------------------------
// ZigChain factory: denom-admin handover
//
// Admin transfer is a two-step propose / claim flow. We only need the propose
// side here — the new admin signs `MsgClaimDenomAdmin` directly from their
// own address, never through this contract.
// ---------------------------------------------------------------------------

#[cfg(feature = "zigchain")]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MsgProposeDenomAdmin {
    #[prost(string, tag = "1")]
    pub signer: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub denom: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub bank_admin: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub metadata_admin: ::prost::alloc::string::String,
}

#[cfg(feature = "zigchain")]
impl MsgProposeDenomAdmin {
    pub const TYPE_URL: &'static str = "/zigchain.factory.MsgProposeDenomAdmin";
}

#[cfg(feature = "zigchain")]
impl TryFrom<Binary> for MsgProposeDenomAdmin {
    type Error = StdError;
    fn try_from(binary: Binary) -> Result<Self, Self::Error> {
        Self::decode(binary.as_slice()).map_err(|e| {
            StdError::generic_err(format!(
                "MsgProposeDenomAdmin Unable to decode binary: \n  - base64: {}\n  - bytes array: {:?}\n\n{:?}",
                binary,
                binary.to_vec(),
                e
            ))
        })
    }
}

/// Propose new bank- and metadata-admin addresses for a denom we currently
/// administer. The new admins must subsequently call `MsgClaimDenomAdmin`
/// themselves from their own addresses — that step is intentionally outside
/// this contract.
#[cfg(feature = "zigchain")]
pub fn tf_propose_denom_admin_msg<T>(
    sender: impl Into<String>,
    denom: impl Into<String>,
    bank_admin: impl Into<String>,
    metadata_admin: impl Into<String>,
) -> CosmosMsg<T>
where
    T: CustomMsg,
{
    let msg = MsgProposeDenomAdmin {
        signer: sender.into(),
        denom: denom.into(),
        bank_admin: bank_admin.into(),
        metadata_admin: metadata_admin.into(),
    };

    CosmosMsg::Stargate {
        type_url: MsgProposeDenomAdmin::TYPE_URL.to_string(),
        value: Binary::from(msg.encode_to_vec()),
    }
}
