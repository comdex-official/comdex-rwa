use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Env, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

pub use cw721_metadata_onchain::{InvestorToken, LendInfo};

use crate::{credit_line::CreditLine, error::ContractResult, ContractError};

#[cw_serde]
pub struct Config {
    pub pool_id: u64,
    pub token_issuer: Addr,
    pub token_id: u128,
    pub admins: Vec<Addr>,
    pub grace_period: Option<u64>,
}

#[cw_serde]
pub struct TranchePool {
    pub pool_id: u64,
    pub pool_name: String,
    pub borrower_addr: Addr,
    pub borrower_name: String,
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
        pool_name: String,
        borrower: Addr,
        borrower_name: String,
        drawdown_period: u64,
        grace_period: u64,
        credit_line: CreditLine,
        env: &Env,
    ) -> Self {
        TranchePool {
            pool_id,
            pool_name,
            borrower_addr: borrower,
            borrower_name,
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
        if env.block.time > self.credit_line.term_start {
            return Err(ContractError::CustomError {
                msg: "Deposits not allowed after term start".to_string(),
            });
        }
        let junior_principal = self.junior_tranche.principal_deposited;
        self.junior_tranche.principal_deposited = junior_principal.checked_add(amount)?;

        Ok(())
    }

    pub fn drawdown(&mut self, amount: Uint128, env: &Env) -> ContractResult<()> {
        let total_principal = self
            .junior_tranche
            .principal_deposited
            .checked_sub(self.junior_tranche.principal_redeemed)?
            + self
                .senior_tranche
                .principal_deposited
                .checked_sub(self.senior_tranche.principal_redeemed)?;
        self.credit_line.drawdown(amount, total_principal, env)?;
        self.drawdown_info = Some(env.block.time);
        Ok(())
    }

    pub fn repay(&mut self, amount: &mut Uint128, env: &Env) -> ContractResult<(Uint128, Uint128)> {
        let interest_owed = self.credit_line.interest_owed(env)?;
        let principal_owed = self.credit_line.principal_owed(env)?;
        let repay_interest = if *amount >= interest_owed {
            interest_owed
        } else {
            *amount
        };
        *amount = amount.saturating_sub(repay_interest);
        let repay_principal = if *amount >= principal_owed {
            principal_owed
        } else {
            *amount
        };
        let pending_amounts = self
            .credit_line
            .repay(repay_interest, repay_principal, env)?;
        Ok(pending_amounts)
    }

    pub fn withdraw_max(&mut self, investor_token: &mut InvestorToken) -> ContractResult<Uint128> {
        let (withdrawable_interest, withdrawable_principal) = self
            .credit_line
            .redeemable_interest_and_amount(&investor_token.lend_info)?;

        let amount = withdrawable_interest.checked_add(withdrawable_principal)?;
        Ok(amount)
    }

    pub fn available_to_withdraw(
        &self,
        lend_info: &LendInfo,
        env: &Env,
    ) -> ContractResult<(Uint128, Uint128)> {
        if env.block.time < self.credit_line.term_start {
            return Ok((Uint128::zero(), Uint128::zero()));
        }
        Ok(self.credit_line.redeemable_interest_and_amount(lend_info)?)
    }

    pub fn expected_share_price(&mut self, amount: Uint128) -> ContractResult<Decimal> {
        let principal_deposited = self
            .junior_tranche
            .principal_deposited
            .checked_add(self.senior_tranche.principal_deposited)?;
        let share_price = CreditLine::usdc_to_share_price(amount, principal_deposited)?;
        self.scale_by_percent_ownership(share_price)
    }

    pub fn scale_by_percent_ownership(&self, share_price: Decimal) -> ContractResult<Decimal> {
        let total_deposited = self
            .junior_tranche
            .principal_deposited
            .checked_add(self.senior_tranche.principal_deposited)?;
        self.scale_by_fraction(
            share_price,
            total_deposited,
            self.credit_line.borrow_info.total_borrowed,
        )
    }

    pub fn scale_by_fraction(
        &self,
        amount: Decimal,
        total_deposited: Uint128,
        total_deployed: Uint128,
    ) -> ContractResult<Decimal> {
        let deposited = Decimal::new(total_deposited);
        let deployed = Decimal::new(total_deployed);
        let result = deployed.checked_div(deposited)?.checked_mul(amount)?;

        Ok(result)
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
            PaymentFrequency::Monthly => 30u64 * 3600u64 * 24,
            PaymentFrequency::Quaterly => 90u64 * 3600u64 * 24,
            PaymentFrequency::Biannually => 180u64 * 3600u64 * 24,
            PaymentFrequency::Annually => 360u64 * 3600u64 * 24,
        }
    }
}

/// Access Control Info
#[cw_serde]
pub struct ACI {
    /// max borrow amount
    pub borrow_limit: Addr,
    /// number of pools that the borrower can create
    pub pool_auth: u64,
}

pub const CONFIG: Item<Config> = Item::new("pool_config");
pub const TRANCHE_POOLS: Map<u64, TranchePool> = Map::new("tranche_pools");
pub const BORROWERS: Map<Addr, ACI> = Map::new("borrowers");
pub const WHITELISTED_TOKENS: Map<String, bool> = Map::new("whitelisted_tokens");
pub const USDC: Item<String> = Item::new("usdc_denom");
pub const KYC: Map<Addr, bool> = Map::new("user_kyc");
