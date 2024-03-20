use cosmwasm_std::{Env, Timestamp, Uint128};

use crate::error::{ContractError, ContractResult};
use crate::state::{BorrowInfo, CreditLine, PaymentFrequency, RepaymentInfo};
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
        let mut borrow_info = BorrowInfo::default();
        borrow_info.borrow_limit = borrow_limit;
        CreditLine {
            term_start: Timestamp::default(),
            term_end: Timestamp::default(),
            term_length,
            grace_period,
            principal_grace_period,
            drawdown_period,
            borrow_info,
            interest_apr,
            junior_fee_percent,
            late_fee_apr,
            interest_frequency,
            principal_frequency,
            interest_accrued: Uint128::zero(),
            interest_owed: Uint128::zero(),
            last_full_payment: Timestamp::default(),
            last_update_ts: env.block.time,
        }
    }

    pub fn set_limit(&mut self, amount: Uint128) -> ContractResult<()> {
        if amount > self.borrow_info.borrow_limit {
            return Err(ContractError::CustomError {
                msg: "New limit cannot exceed max limit".to_string(),
            });
        }
        self.borrow_info.current_limit = amount;
        Ok(())
    }

    pub fn limit(&self, env: &Env) -> ContractResult<Uint128> {
        Ok(self
            .borrow_info
            .current_limit
            .checked_sub(self.principal_owed(env)?)?)
    }

    pub fn max_limit(&self) -> Uint128 {
        self.borrow_info.borrow_limit
    }

    pub fn drawdown(&mut self, amount: Uint128, env: &Env) -> ContractResult<()> {
        let total_amount = self.borrow_info.borrowed_amount.checked_add(amount)?;
        let limit = self.limit(env)?;
        if total_amount > limit {
            return Err(ContractError::DrawdownExceedsLimit { limit });
        };

        if self.borrow_info.borrowed_amount.is_zero() {
            self.last_full_payment = env.block.time;
            if self.term_start == Timestamp::default() {
                self.term_start = env.block.time.plus_seconds(self.drawdown_period);
                self.term_end = self.term_start.plus_seconds(self.term_length);
            }
        }

        self.checkpoint(env)?;
        self.borrow_info.borrowed_amount = total_amount;
        self.borrow_info.total_borrowed = self.borrow_info.total_borrowed.checked_add(amount)?;
        if !self.is_late(env)? {
            return Err(ContractError::CustomError {
                msg: "Drawdown not allowed when payments are due".to_string(),
            });
        }
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
        if self.term_start == Timestamp::default() {
            return Err(ContractError::CustomError {
                msg: "Term not started".to_string(),
            });
        }
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
        if self.term_start == Timestamp::default() {
            return Err(ContractError::CustomError {
                msg: "Term not started".to_string(),
            });
        }
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
        if self.term_start == Timestamp::default() {
            return Err(ContractError::CustomError {
                msg: "Term not started".to_string(),
            });
        }
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
        if self.term_start == Timestamp::default() {
            return Err(ContractError::CustomError {
                msg: "Term not started".to_string(),
            });
        }
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
        if self.term_start == Timestamp::default() {
            return Ok(Uint128::zero());
        }
        if env.block.time < self.term_start {
            return Ok(Uint128::zero());
        }
        let past_interest = self.interest_accrued;
        let latest_interest =
            self.interest_over_period(self.last_update_ts, env.block.time, env)?;

        Ok(past_interest + latest_interest)
    }

    pub fn interest_over_period(
        &self,
        begin: Timestamp,
        end: Timestamp,
        env: &Env,
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
        let late_fee = self.late_fee_accrued_over_period(begin, end, env)?;

        Ok(interest + late_fee)
    }

    pub fn late_fee_accrued_over_period(
        &self,
        begin: Timestamp,
        end: Timestamp,
        env: &Env,
    ) -> ContractResult<Uint128> {
        let mut _env = env.to_owned();
        _env.block.time = self.last_full_payment;
        let oldest_unpaid_date = self.next_due_date(&_env)?;
        let late_fee_start =
            std::cmp::max(begin, oldest_unpaid_date.plus_seconds(self.grace_period));

        if late_fee_start > end {
            return Ok(Uint128::zero());
        }
        let period = end.seconds() - late_fee_start.seconds();
        Ok(self
            .borrow_info
            .borrowed_amount
            .checked_mul(Uint128::new(self.late_fee_apr as u128))?
            .checked_div(SIY)?
            .checked_mul(Uint128::new(period as u128))?)
    }

    pub fn _interest_owed(&self, env: &Env) -> ContractResult<Uint128> {
        if self.term_start == Timestamp::default() {
            return Ok(Uint128::zero());
        }
        if env.block.time < self.term_start {
            return Ok(Uint128::zero());
        }
        if env.block.time > self.term_end {
            return Ok(self.interest_accrued
                + self.interest_over_period(self.last_update_ts, env.block.time, env)?);
        }
        let prev_interest_due_date = self.prev_interest_due_date(env)?;
        if self.last_update_ts <= prev_interest_due_date && prev_interest_due_date <= env.block.time
        {
            return Ok(self.interest_accrued
                + self.interest_over_period(self.last_update_ts, env.block.time, env)?);
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
        if self.term_start == Timestamp::default() {
            return Ok(Uint128::zero());
        }
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
        Ok(self
            ._interest_owed(env)?
            .saturating_sub(self.borrow_info.interest_repaid))
    }

    pub fn interest_accrued(&self, env: &Env) -> ContractResult<Uint128> {
        Ok(self._interest_accrued(env)?)
    }

    pub fn principal_owed(&self, env: &Env) -> ContractResult<Uint128> {
        Ok(self
            ._principal_owed(env)?
            .saturating_sub(self.borrow_info.principal_repaid))
    }

    pub fn is_late(&self, env: &Env) -> ContractResult<bool> {
        let mut _env = env.to_owned();
        _env.block.time = self.last_full_payment;
        let next_due_date = self.next_due_date(&_env)?;
        Ok(self.borrow_info.borrowed_amount.u128() > 0u128 && env.block.time > next_due_date)
    }

    pub fn repay(&mut self, mut amount: Uint128, env: &Env) -> ContractResult<RepaymentInfo> {
        if amount.is_zero() {
            return Err(ContractError::CustomError {
                msg: "repayment amount is zero".to_string(),
            });
        }

        let mut repayment_info = RepaymentInfo::default();
        repayment_info.timestamp = env.block.time;

        self.checkpoint(env)?;

        // repay principal and interest
        let current_interest_owed = self.interest_owed(env)?;
        let current_principal_owed = self.principal_owed(env)?;
        if amount >= current_interest_owed {
            self.borrow_info
                .interest_repaid
                .checked_add(current_interest_owed)?;
            repayment_info.interest_repaid = current_interest_owed;
            amount -= current_interest_owed;
        } else {
            self.borrow_info.interest_repaid.checked_add(amount)?;
            repayment_info.interest_repaid = amount;
            repayment_info.interest_pending = current_interest_owed - amount;
        }
        if amount >= current_principal_owed {
            self.borrow_info
                .principal_repaid
                .checked_add(current_interest_owed)?;
            repayment_info.principal_repaid = current_principal_owed;
            amount -= current_principal_owed;
        } else {
            self.borrow_info.principal_repaid.checked_add(amount)?;
            repayment_info.principal_repaid = amount;
            repayment_info.principal_pending = current_principal_owed - amount;
        }

        self.borrow_info.interest_repaid = self
            .borrow_info
            .interest_repaid
            .checked_add(repayment_info.interest_repaid)?;
        self.borrow_info.principal_repaid = self
            .borrow_info
            .principal_repaid
            .checked_add(repayment_info.principal_repaid)?;

        // if both owed amounts in now zero, update last_full_payment
        if repayment_info.interest_pending.is_zero() && repayment_info.principal_pending.is_zero() {
            self.last_full_payment = env.block.time;
        }
        Ok(repayment_info)
    }
}
