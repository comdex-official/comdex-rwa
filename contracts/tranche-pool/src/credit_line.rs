use cosmwasm_std::{Decimal, Env, Timestamp, Uint128};

use crate::error::{ContractError, ContractResult};
use crate::helpers::share_price_to_usdc;
use crate::state::{CreditLine, LendInfo, PaymentFrequency};
use crate::{SIY, TEN_THOUSAND};

impl CreditLine {
    pub fn new(
        borrow_limit: Uint128,
        term_length: u64,
        drawdown_period: u64,
        grace_period: u64,
        principal_grace_period: u64,
        interest_apr: u16,
        late_fee_apr: u16,
        junior_fee_percent: u16,
        interest_frequency: PaymentFrequency,
        principal_frequency: PaymentFrequency,
        env: &Env,
    ) -> Self {
        let mut credit_line = CreditLine::default();
        credit_line.grace_period = grace_period;
        credit_line.principal_frequency = principal_frequency;
        credit_line.interest_frequency = interest_frequncy;
        credit_line.interest_apr = interest_apr;
        credit_line.late_fee_apr = late_fee_apr;
        credit_line.junior_fee_percent = junior_fee_percent;
        credit_line.principal_grace_period = principal_grace_period;
        credit_line.last_update_ts = env.block.time;
        credit_line.borrow_info.borrow_limit = borrow_limit;
        credit_line.borrow_info.principal_share_price = Decimal::one();
        credit_line.borrow_info.interest_share_price = Decimal::one();
        credit_line
    }

    pub fn drawdown(
        &mut self,
        amount: Uint128,
        total_principal: Uint128,
        env: &Env,
    ) -> ContractResult<()> {
        let total_amount = self.borrow_info.borrowed_amount.checked_add(amount)?;
        if total_amount > self.borrow_info.borrow_limit {
            return Err(ContractError::DrawdownExceedsLimit {
                limit: self.borrow_info.borrow_limit,
            });
        };
        if total_amount > total_principal {
            return Err(ContractError::DrawdownExceedsLimit {
                limit: total_principal,
            });
        }

        if let Some(&term_start) = self.term_start.as_ref() {
            if env.block.time >= term_start {
                return Err(ContractError::NotInDrawdownPeriod);
            }
        }

        self.checkpoint(env)?;
        self.borrow_info.borrowed_amount = total_amount;
        self.borrow_info.total_borrowed = self.borrow_info.total_borrowed.checked_add(amount)?;
        Ok(())
    }

    pub fn next_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        let _interest_due_date = self.next_interest_due_date(env)?;
        let _principal_due_date = self.next_principal_due_date(env)?;
        if _interest_due_date < _principal_due_date {
            return Ok(_interest_due_date);
        };
        Ok(_principal_due_date)
    }

    pub fn prev_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        let _interest_due_date = self.prev_interest_due_date(env)?;
        let _principal_due_date = self.prev_principal_due_date(env)?;
        if _interest_due_date > _principal_due_date {
            return Ok(_interest_due_date);
        }
        Ok(_principal_due_date)
    }

    pub fn next_interest_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        if env.block.time < self.term_start {
            return Ok(self
                .term_start
                .plus_seconds(self.interest_frequency.to_seconds()));
        };
        let seconds_till_date = env
            .block
            .time
            .minus_seconds(self.term_start.seconds())
            .seconds();
        let periods_passed = seconds_till_date / self.interest_frequency.to_seconds();
        let abs_seconds = periods_passed * self.interest_frequency.to_seconds();
        let diff = seconds_till_date - abs_seconds;
        if diff == 0 {
            return Ok(self.term_start.plus_seconds(abs_seconds));
        }
        Ok(self
            .term_start
            .plus_seconds((periods_passed + 1) * self.interest_frequency.to_seconds()))
    }

    pub fn next_principal_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        if env.block.time < (self.term_start.plus_seconds(self.principal_grace_period)) {
            return Ok(self
                .term_start
                .plus_seconds(self.principal_grace_period)
                .plus_seconds(self.principal_frequency.to_seconds()));
        };
        let seconds_till_date = env
            .block
            .time
            .minus_seconds(
                self.term_start
                    .plus_seconds(self.principal_grace_period)
                    .seconds(),
            )
            .seconds();
        let periods_passed = seconds_till_date / self.principal_frequency.to_seconds();
        let abs_seconds = periods_passed * self.interest_frequency.to_seconds();
        let diff = seconds_till_date - abs_seconds;
        if diff == 0 {
            return Ok(self.term_start.plus_seconds(abs_seconds));
        }
        Ok(self
            .term_start
            .plus_seconds(abs_seconds + self.principal_frequency.to_seconds()))
    }

    pub fn prev_interest_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        if env.block.time < self.term_start {
            return Err(ContractError::CustomError {
                msg: "Term not started yet".to_string(),
            });
        }
        let seconds_till_date = env
            .block
            .time
            .minus_seconds(self.term_start.seconds())
            .seconds();
        let periods_passed = seconds_till_date / self.principal_frequency.to_seconds();

        Ok(self
            .term_start
            .plus_seconds(periods_passed * self.principal_frequency.to_seconds()))
    }

    pub fn prev_principal_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        if env.block.time < self.term_start.plus_seconds(self.principal_grace_period) {
            return Err(ContractError::CustomError {
                msg: "Within principal grace period".to_string(),
            });
        };
        let seconds_till_date = env
            .block
            .time
            .minus_seconds(
                self.term_start
                    .plus_seconds(self.principal_grace_period)
                    .seconds(),
            )
            .seconds();
        let periods_passed = seconds_till_date / self.principal_frequency.to_seconds();

        Ok(self
            .term_start
            .plus_seconds(periods_passed * self.principal_frequency.to_seconds()))
    }

    pub fn _interest_accrued(&self, env: &Env) -> ContractResult<Uint128> {
        if env.block.time < self.term_start {
            return Ok(Uint128::zero());
        }
        let past_interest = self.interest_accrued;
        let latest_interest = self.interest_over_period(self.last_update_ts, env.block.time)?;

        Ok(past_interest + latest_interest)
    }

    pub fn interest_over_period(
        &self,
        begin: Timestamp,
        end: Timestamp,
    ) -> ContractResult<Uint128> {
        if end < begin {
            return Err(ContractError::CustomError {
                msg: "end timestamp is smaller than begin timestamp".to_string(),
            });
        }
        let period = end.minus_seconds(begin.seconds()).seconds();
        // (((borrow_amount / 10000) * interest) / 365*24*3600) * duration of borrow
        let interest = self
            .borrow_info
            .borrowed_amount
            .checked_div(TEN_THOUSAND)?
            .checked_mul(Uint128::from(self.interest_apr))?
            .checked_div(SIY)?
            .checked_mul(Uint128::from(period))?;
        // !-------
        // calculate late fee
        // -------!

        Ok(interest)
    }

    pub fn _interest_owed(&self, env: &Env) -> ContractResult<Uint128> {
        if env.block.time < self.term_start {
            return Ok(Uint128::zero());
        }
        if env.block.time > self.term_end {
            return Ok(self.interest_accrued
                + self.interest_over_period(self.last_update_ts, env.block.time)?);
        }
        let prev_interest_due_date = self.prev_interest_due_date(env)?;
        if self.last_update_ts <= prev_interest_due_date && prev_interest_due_date <= env.block.time
        {
            return Ok(self.interest_accrued
                + self.interest_over_period(self.last_update_ts, env.block.time)?);
        }
        Ok(self.interest_owed)
    }

    pub fn checkpoint(&mut self, env: &Env) -> ContractResult<()> {
        self.interest_accrued = self._interest_accrued(env)?;
        self.interest_owed = self._interest_owed(env)?;
        self.last_update_ts = env.block.time;
        Ok(())
    }

    pub fn _principal_owed(&self, env: &Env) -> ContractResult<Uint128> {
        if env.block.time < self.term_start.plus_seconds(self.principal_grace_period) {
            return Ok(Uint128::zero());
        }
        let periods_passed = env
            .block
            .time
            .minus_seconds(
                self.term_start
                    .plus_seconds(self.principal_grace_period)
                    .seconds(),
            )
            .seconds()
            / self.principal_frequency.to_seconds();
        let total_principal_payments = self
            .term_end
            .minus_seconds(self.term_start.seconds())
            .seconds()
            / self.principal_frequency.to_seconds();
        Ok(self
            .borrow_info
            .total_borrowed
            .checked_div(Uint128::from(total_principal_payments))?
            .checked_mul(Uint128::from(periods_passed))?)
    }

    pub fn interest_owed(&self, env: &Env) -> ContractResult<Uint128> {
        return Ok(self
            ._interest_owed(env)?
            .saturating_sub(self.borrow_info.interest_repaid));
    }

    pub fn interest_accrued(&self, env: &Env) -> ContractResult<Uint128> {
        return Ok(self._interest_accrued(env)?);
    }

    pub fn principal_owed(&self, env: &Env) -> ContractResult<Uint128> {
        return Ok(self
            ._principal_owed(env)?
            .saturating_sub(self.borrow_info.principal_repaid));
    }

    pub fn is_late(&self, env: &Env) -> ContractResult<bool> {
        let mut _env = env.to_owned();
        _env.block.time = self.last_full_payment;
        let next_due_date = self.next_due_date(&_env)?;
        Ok(self.borrow_info.borrow_limit.u128() > 0u128 && env.block.time > next_due_date)
    }

    pub fn repay(
        &mut self,
        repay_interest: Uint128,
        repay_principal: Uint128,
        env: &Env,
    ) -> ContractResult<(Uint128, Uint128)> {
        if repay_interest == Uint128::zero() && repay_principal == Uint128::zero() {
            return Err(ContractError::CustomError {
                msg: "repayment amount is zero".to_string(),
            });
        }
        self.checkpoint(env)?;
        // repay principal and interest
        let current_interest_owed = self.interest_owed(env)?;
        let current_principal_owed = self.principal_owed(env)?;
        let interest_pending = Uint128::zero();
        let principal_pending = Uint128::zero();
        if repay_interest >= current_interest_owed {
            self.borrow_info
                .interest_repaid
                .checked_add(current_interest_owed)?;
        } else {
            self.borrow_info
                .interest_repaid
                .checked_add(repay_interest)?;
            interest_pending.checked_add(current_interest_owed - repay_interest)?;
        }
        if repay_principal >= current_principal_owed {
            self.borrow_info
                .principal_repaid
                .checked_add(current_interest_owed)?;
        } else {
            self.borrow_info
                .principal_repaid
                .checked_add(repay_principal)?;
            principal_pending.checked_add(current_principal_owed - repay_principal)?;
        }
        // if both owed amounts in now zero, update last_full_payment
        if interest_pending == Uint128::zero() && principal_pending == Uint128::zero() {
            self.last_full_payment = env.block.time;
        }
        Ok((interest_pending, principal_pending))
    }

    pub fn redeemable_interest_and_amount(
        &self,
        lend_info: &LendInfo,
    ) -> ContractResult<(Uint128, Uint128)> {
        let max_principal_redeemable = share_price_to_usdc(
            self.borrow_info.principal_share_price,
            lend_info.principal_deposited,
        )?;
        let max_interest_redeemable = share_price_to_usdc(
            self.borrow_info.interest_share_price,
            lend_info.principal_deposited,
        )?;
        let redeemable_principal =
            max_principal_redeemable.checked_sub(lend_info.principal_redeemed)?;
        let redeemable_interest =
            max_interest_redeemable.checked_sub(lend_info.interest_redeemed)?;
        Ok((redeemable_interest, redeemable_principal))
    }
}
