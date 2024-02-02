use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;
use crate::state::PaymentFrequency;

#[cw_serde]
pub struct InstantiateMsg {
    pub admins: Vec<String>,
    pub token_issuer: String,
    pub usdc_denom: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    NewPool { msg: CreatePoolMsg },
}

#[cw_serde]
pub struct CreatePoolMsg {
    pub borrower: String,
    pub uid_token: Uint128,
    pub interest_apr: u16,
    pub borrow_limit: Uint128,
    pub interest_payment_frequency: PaymentFrequency,
    pub principal_payment_frequency: PaymentFrequency,
    pub principal_grace_period: u64,
    pub drawdown_period: u64,
    pub term_length: u64,
}

#[cw_serde]
pub struct DepositMsg {
    pub amount: Uint128,
    pub pool_id: u64,
}

#[cw_serde]
pub struct DrawdownMsg {
    pub pool_id: u64,
    pub amount: Uint128,
}

pub struct RepayMsg {
    pub pool_id: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub enum QueryMsg {
    Query1 {},
}
