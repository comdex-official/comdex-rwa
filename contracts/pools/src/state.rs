use crate::{error::ContractResult, ContractError};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Env, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

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
    pub interest_apy: u16,
    pub borrower_addr: Addr,
    pub creation_info: Timestamp,
    pub payment_schedule: PaymentFrequency,
    pub drawdown_info: Option<Timestamp>,
    pub drawdown_period: u64,
    pub grace_period: u64,
    pub junior_tranche: LendInfo,
    pub senior_tranche: LendInfo,
    pub backers: Vec<u128>,
    pub repayment_info: TermInfo,
}

#[cw_serde]
#[derive(Default)]
pub struct TermInfo {
    /// Calculates interest from this point in time
    pub term_start: Timestamp,
    /// Last interest payment date, i.e. closing of pool
    pub term_end: Timestamp,
    /// the next date of payment
    pub next_due: Timestamp,
    /// date of lastest payment
    pub lastest_payment: Timestamp,
    pub last_update_ts: Timestamp,
}

impl TranchePool {
    pub fn new(
        pool_id: u64,
        borrow_limit: Uint128,
        interest_apy: u16,
        borrower: Addr,
        payment_schedule: PaymentFrequency,
        env: &Env,
    ) -> Self {
        TranchePool {
            pool_id,
            interest_apy,
            borrower_addr: borrower,
            creation_info: env.block.time,
            payment_schedule,
            drawdown_info: None,
            grace_period: None,
            junior_tranche: LendInfo::default(),
            senior_tranche: LendInfo::default(),
            backers: Vec::new(),
            repayment_info: TermInfo::default(),
        }
    }

    pub fn set_grace_period(&mut self, new_grace_period: BlockInfo) {
        self.grace_period = Some(new_grace_period);
    }

    pub fn deposit(&mut self, amount: Uint128) -> ContractResult<()> {
        self.junior_tranche.principal_deposited.checked_add(amount);

        Ok(())
    }

    pub fn drawdown(&mut self, amount: Uint128, env: &Env) -> ContractResult<()> {
        let borrow_info = &mut self.borrow_info;
        if borrow_info.borrowed_amount + amount > borrow_info.borrow_limit {
            return Err(ContractError::DrawdownExceedsLimit {
                limit: borrow_info.borrow_limit,
            });
        };
        if self.drawdown_info.is_none() {
            let repayment_info = &mut self.repayment_info;
            repayment_info.first_drawdown = env.block.time;
            repayment_info.last_update_ts = env.block.time;
            repayment_info.next_due = env.block.time.plus_days(30);
        } else {
            // checkpoint
        }
        self.drawdown_info = Some(BlockInfo::new(env));
        Ok(())
    }

    pub fn checkpoint(&mut self, env: &Env) -> ContractResult<()> {
        // update interest accrued
        self.update_interest_accrued(env)?;
        // update interest owed
        // update timestamp

        Ok(())
    }

    pub fn interest_accrued(&self, env: &Env) -> ContractResult<Uint128> {
        let past_interest = self.borrow_info.interest_accrued;
        let period = env.block.time.seconds() - self.borrow_info.last_update_ts.seconds();
        // (((borrow_amount / 10000) * interest) / 365*24*3600) * duration of borrow
        let latest_interest = self
            .borrow_info
            .borrowed_amount
            .checked_div(TEN_THOUSAND)?
            .checked_mul(Uint128::from(self.interest_apy))?
            .checked_div(SIY)?
            .checked_mul(Uint128::from(period))?;
        // !-------
        // calculate late fee
        // -------!

        Ok(past_interest + latest_interest)
    }

    fn update_interest_accrued(&mut self, env: &Env) -> ContractResult<Uint128> {
        self.borrow_info.interest_accrued = self.interest_accrued(env)?;
        self.borrow_info.last_update_ts = env.block.time;

        Ok(self.borrow_info.interest_accrued)
    }

    fn update_interest_owed(&mut self, env: &Env) -> ContractResult<Uint128> {
        let owed = Uint128::zero();
        Ok(owed)
    }

    pub fn next_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        Ok(self.repayment_info.next_due)
    }

    pub fn interest_owed(&self, env: &Env) -> ContractResult<Uint128> {
        if env.block.time > self.repayment_info.term_end {
            return self.interest_accrued(env)
        }

        Ok(Uint128::zero())

    }
}

#[cw_serde]
pub enum PaymentFrequency {
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
