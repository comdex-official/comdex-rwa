use crate::state::PaymentFrequency;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub token_issuer: String,
    pub code_id: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    NewPool { msg: CreatePoolMsg },
    Deposit { msg: DepositMsg },
    Repay { msg: RepayMsg },
    Drawdown { msg: DrawdownMsg },
    AddBackers { backers: Vec<String> },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct CreatePoolMsg {
    pub pool_name: String,
    pub borrower: String,
    pub borrower_name: String,
    pub uid_token: Uint128,
    pub interest_apr: u16,
    pub junior_fee_percent: u16,
    pub late_fee_apr: u16,
    pub borrow_limit: Uint128,
    pub interest_frequency: PaymentFrequency,
    pub principal_frequency: PaymentFrequency,
    pub principal_grace_period: u64,
    pub drawdown_period: u64,
    pub term_length: u64,
    pub denom: String,
    pub backers: Vec<String>,
}

#[cw_serde]
pub struct DepositMsg {
    pub amount: Uint128,
    pub pool_id: u64,
    pub tranche_id: u64,
}

#[cw_serde]
pub struct DrawdownMsg {
    pub pool_id: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub struct LockPoolMsg {
    pub pool_id: u64,
}

#[cw_serde]
pub struct RepayMsg {
    pub pool_id: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub enum QueryMsg {
    GetConfig {},
    GetPoolInfo {
        id: u64,
    },
    GetAllPools {
        start: Option<u64>,
        limit: Option<u8>,
    },
}
