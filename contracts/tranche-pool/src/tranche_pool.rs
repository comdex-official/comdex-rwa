use crate::{
    error::ContractResult,
    helpers::usdc_to_share_price,
    state::{CreditLine, InvestorToken, LendInfo, PoolStatus, TranchePool},
    ContractError,
};
use cosmwasm_std::{Addr, Decimal, Env, Uint128};

impl TranchePool {
    pub fn new(
        pool_id: u64,
        pool_name: String,
        borrower: Addr,
        borrower_name: String,
        denom: String,
        backers: Vec<Addr>,
        env: &Env,
    ) -> Self {
        TranchePool {
            pool_id,
            pool_name,
            borrower_addr: borrower,
            borrower_name,
            creation_info: env.block.time,
            denom,
            backers,
        }
    }

    pub fn is_backer(&self, user: &Addr) -> bool {
        self.backers.iter().any(|backer| backer == user)
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
        let share_price = usdc_to_share_price(amount, principal_deposited)?;
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
