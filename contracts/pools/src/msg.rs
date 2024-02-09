use crate::state::PaymentFrequency;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub admins: Vec<String>,
    pub token_issuer: String,
    pub usdc_denom: String,
    pub code_id: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    NewPool { msg: CreatePoolMsg },
    UpdateKyc { user: Addr, kyc_status: bool },
    Deposit { msg: DepositMsg },
    Repay { msg: RepayMsg },
    Drawdown { msg: DrawdownMsg },
}

#[cw_serde]
pub struct MigrateMsg {}

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

#[cw_serde]
pub struct RepayMsg {
    pub pool_id: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub enum QueryMsg {
    GetConfig {},
    GetPoolInfo { id: u64 },
    CheckKycStatus { user: String },
}
