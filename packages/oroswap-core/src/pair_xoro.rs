use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct XoroPairInitParams {
    pub staking: String,
}
