use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Env, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

use crate::{credit_line::CreditLine, error::ContractResult};

#[cw_serde]
pub struct Config {
    pub pool_id: u64,
    pub token_issuer: Addr,
    pub token_id: u128,
    pub admins: Vec<Addr>,
    pub grace_period: Option<u64>,
}

#[cw_serde]
pub struct LendInfo {
    pub principal_deposited: Uint128,
    pub principal_redeemed: Uint128,
    pub interest_redeemed: Uint128,
}

impl Default for LendInfo {
    fn default() -> Self {
        LendInfo {
            principal_deposited: Uint128::zero(),
            principal_redeemed: Uint128::zero(),
            interest_redeemed: Uint128::zero(),
        }
    }
}

#[cw_serde]
pub struct InvestorToken {
    pub token_id: u128,
    pub pool_id: u64,
    pub lend_info: LendInfo,
}

impl InvestorToken {
    pub fn new(token_id: u128, pool_id: u64) -> Self {
        InvestorToken {
            token_id,
            pool_id,
            lend_info: LendInfo::default(),
        }
    }
}

#[cw_serde]
pub struct TranchePool {
    pub pool_id: u64,
    pub borrower_addr: Addr,
    pub creation_info: Timestamp,
    pub drawdown_info: Option<Timestamp>,
    pub drawdown_period: u64,
    pub grace_period: u64,
    pub junior_tranche: LendInfo,
    pub senior_tranche: LendInfo,
    pub credit_line: CreditLine,
    pub backers: Vec<u128>,
}

impl TranchePool {
    pub fn new(
        pool_id: u64,
        borrow_limit: Uint128,
        borrower: Addr,
        drawdown_period: u64,
        grace_period: u64,
        credit_line: CreditLine,
        env: &Env,
    ) -> Self {
        TranchePool {
            pool_id,
            borrower_addr: borrower,
            creation_info: env.block.time,
            drawdown_info: None,
            drawdown_period,
            grace_period,
            junior_tranche: LendInfo::default(),
            senior_tranche: LendInfo::default(),
            backers: Vec::new(),
            credit_line,
        }
    }

    pub fn set_grace_period(&mut self, new_grace_period: u64) {
        self.grace_period = new_grace_period;
    }

    pub fn deposit(&mut self, amount: Uint128, env: &Env) -> ContractResult<()> {
        self.credit_line.drawdown(amount, env)?;
        Ok(())
    }

    pub fn drawdown(&mut self, amount: Uint128, env: &Env) -> ContractResult<()> {
        self.credit_line.drawdown(amount, env)?;
        Ok(())
    }

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
            PaymentFrequency::Monthly => 30u64 * 3600u64,
            PaymentFrequency::Quaterly => 90u64 * 3600u64,
            PaymentFrequency::Biannually => 180u64 * 3600u64,
            PaymentFrequency::Annually => 360u64 * 3600u64,
        }
    }
}

/// Access Control Info
#[cw_serde]
pub struct ACI {
    pub borrower: Addr,
    /// max borrow amount
    pub borrow_limit: Addr,
    /// number of pools that the borrower can create
    pub pool_auth: u64,
}

pub const CONFIG: Item<Config> = Item::new("pool_config");
pub const TRANCHE_POOLS: Map<u64, TranchePool> = Map::new("tranche_pools");
pub const BORROWERS: Map<Addr, ACI> = Map::new("borrowers");
pub const WHITELISTED_TOKENS: Map<String, bool> = Map::new("whitelisted_tokens");
