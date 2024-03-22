use crate::state::{PaymentFrequency, PoolType};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub code_id: u64,
    pub reserves_fee: u16,
    pub reserves_addr: String,
    pub kyc_addr: String,
    pub denom: String,
    pub decimals: u8,
}

#[cw_serde]
pub enum ExecuteMsg {
    NewPool { msg: CreatePoolMsg },
    Deposit { msg: DepositMsg },
    Repay { msg: RepayMsg },
    Drawdown { msg: DrawdownMsg },
    AddBackers { backers: Vec<String> },
    LockPool { msg: LockPoolMsg },
    LockJuniorCapital { msg: LockJuniorCapitalMsg },
    SetKycContract { addr: String },
    WhitelistToken { denom: String, decimals: u8 },
    SetSeniorPool { denom: String, addr: String },
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
    pub pool_type: PoolType,
}

#[cw_serde]
pub struct DepositMsg {
    pub pool_id: u64,
    pub tranche_id: u64,
    pub amount: Uint128,
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
pub struct LockJuniorCapitalMsg {
    pub pool_id: u64,
}

#[cw_serde]
pub struct RepayMsg {
    pub pool_id: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub struct WithdrawMsg {
    pub token_id: u64,
    pub amount: Option<Uint128>,
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
    RepaymentInfo {
        id: u64,
    },
}

#[cw_serde]
pub struct PoolInfo {
    pub pool_id: u64,
    pub pool_name: String,
    pub borrower_name: String,
    pub borrower: String,
    pub assets: Uint128,
    pub asset_info: Option<rwa_core::state::Asset>,
    pub apr: Decimal,
    pub pool_type: PoolType,
    pub status: String,
    pub invested: Uint128,
    pub drawn: Uint128,
    pub available_to_draw: Uint128,
    pub interest_paid: Uint128,
    pub interest_accrued: Uint128,
    pub interest_pending: Uint128,
    pub tranche_id: String,
}

#[cw_serde]
pub struct PoolResponse {
    pub pool_id: u64,
    pub pool_name: String,
    pub borrower_name: String,
    pub borrower: String,
    pub assets: Uint128,
    pub denom: String,
    pub decimals: u32,
    pub apr: Decimal,
    pub pool_type: PoolType,
    pub status: String,
}

#[cw_serde]
pub struct AllPoolsResponse {
    pub data: Vec<PoolResponse>,
}

#[cw_serde]
pub struct PaymentInfo {
    pub addr: Addr,
    pub paid: Uint128,
    pub expected: Uint128,
}
