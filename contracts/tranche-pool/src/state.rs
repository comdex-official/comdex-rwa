use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

pub use cw721_metadata_onchain::{InvestorToken, LendInfo};

#[cw_serde]
pub struct Config {
    pub pool_id: u64,
    pub token_issuer: Addr,
    pub token_id: u128,
    pub admin: Addr,
    pub grace_period: Option<u64>,
    pub reserve_fee: u16,
}

#[cw_serde]
pub struct TranchePool {
    pub pool_id: u64,
    pub pool_name: String,
    pub borrower_addr: Addr,
    pub borrower_name: String,
    pub creation_info: Timestamp,
    pub pool_type: PoolType,
    pub denom: String,
    pub backers: Vec<Addr>,
}

#[cw_serde]
pub enum PoolStatus {
    Open,
    Late,
    Closed,
}

#[cw_serde]
pub enum PoolType {
    Undefined,
    Junior
}

#[cw_serde]
#[derive(Default)]
pub struct BorrowInfo {
    pub current_limit: Uint128,
    pub borrow_limit: Uint128,
    pub total_borrowed: Uint128,
    pub borrowed_amount: Uint128,
    pub interest_repaid: Uint128,
    pub principal_repaid: Uint128,
}

#[cw_serde]
pub struct TrancheInfo {
    pub id: u64,
    pub principal_deposited: Uint128,
    pub principal_share_price: Decimal,
    pub interest_share_price: Decimal,
    pub locked_until: Timestamp,
}

#[cw_serde]
pub struct PoolSlice {
    pub junior_tranche: TrancheInfo,
    pub senior_tranche: TrancheInfo,
    pub total_interest_accrued: Uint128,
    pub principal_deployed: Uint128,
}

#[cw_serde]
pub struct CreditLine {
    /// Prior this date, no interest is charged
    pub term_start: Timestamp,
    /// Post this date, all accrued interest is due
    pub term_end: Timestamp,
    pub term_length: u64,
    /// Grace period post due date
    pub grace_period: u64,
    /// Initial grace period for principal repayment
    pub principal_grace_period: u64,
    pub drawdown_period: u64,
    pub borrow_info: BorrowInfo,
    /// 12.50% interest is represented as 1250
    pub interest_apr: u16,
    pub junior_fee_percent: u16,
    pub late_fee_apr: u16,
    pub interest_frequency: PaymentFrequency,
    pub principal_frequency: PaymentFrequency,
    pub interest_accrued: Uint128,
    pub interest_owed: Uint128,
    pub last_full_payment: Timestamp,
    pub last_update_ts: Timestamp,
}

#[cw_serde]
#[derive(Default)]
pub enum PaymentFrequency {
    #[default]
    Monthly,
    Quaterly,
    Biannually,
    Annually,
}

impl PaymentFrequency {
    pub fn to_seconds(&self) -> u64 {
        match self {
            PaymentFrequency::Monthly => 30u64 * 3600u64 * 24,
            PaymentFrequency::Quaterly => 90u64 * 3600u64 * 24,
            PaymentFrequency::Biannually => 180u64 * 3600u64 * 24,
            PaymentFrequency::Annually => 360u64 * 3600u64 * 24,
        }
    }
}

#[cw_serde]
pub struct AllPoolsResponse {
    pub pool_id: u64,
    pub pool_name: String,
    pub borrower_name: String,
    pub interest_apr: u16,
    pub tvl: Uint128,
}

/// Access Control Info
#[cw_serde]
pub struct ACI {
    /// max borrow amount
    pub borrow_limit: Addr,
    /// number of pools that the borrower can create
    pub pool_auth: u64,
}

#[cw_serde]
#[derive(Default)]
pub struct RepaymentInfo {
    pub timestamp: Timestamp,
    pub principal_repaid: Uint128,
    pub interest_repaid: Uint128,
    pub principal_pending: Uint128,
    pub interest_pending: Uint128,
    pub excess_amount: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("pool_config");
pub const KYC_CONTRACT: Item<Addr> = Item::new("kyc_contract");
pub const TRANCHE_POOLS: Map<u64, TranchePool> = Map::new("tranche_pools");
pub const CREDIT_LINES: Map<u64, CreditLine> = Map::new("credit_lines");
pub const POOL_SLICES: Map<u64, Vec<PoolSlice>> = Map::new("pool_slices");
pub const BORROWERS: Map<Addr, ACI> = Map::new("borrowers");
pub const WHITELISTED_TOKENS: Map<String, (bool, u32)> = Map::new("whitelisted_tokens");
pub const RESERVE_ADDR: Item<Addr> = Item::new("reserves_addr");
pub const SENIOR_POOLS: Map<String, Addr> = Map::new("senior_pools");
pub const PAUSED: Item<bool> = Item::new("drawdown_pause");
pub const REPAYMENTS: Map<u64, Vec<RepaymentInfo>> = Map::new("reapymanet_history");
