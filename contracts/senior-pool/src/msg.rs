use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub cw20_code_id: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    Tx1 {},
}

#[cw_serde]
pub enum QueryMsg {}

#[cw_serde]
pub struct DepositMsg {
    pub amount: Uint128,
}
