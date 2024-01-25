use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Env, Timestamp, Uint128};

use crate::error::ContractResult;
use crate::state::PaymentFrequency;
use crate::{SIY, TEN_THOUSAND};

#[cw_serde]
pub struct BorrowInfo {
    pub borrow_limit: Uint128,
    pub borrowed_amount: Uint128,
    pub interest_repaid: Uint128,
    pub principal_repaid: Uint128,
}

#[cw_serde]
pub struct CreditLine {
    /// Prior this date, no interest is charged
    pub term_start: Timestamp,
    /// Post this date, all accrued interest is due
    pub term_end: Timestamp,
    /// Grace period post due date
    pub grace_period: u64,
    /// Initial grace period for principal repayment
    pub principal_grace_period: u64,
    pub borrow_info: BorrowInfo,
    /// 12.50% interest is represented as 1250
    pub interest_apr: u16,
    pub interest_frequency: PaymentFrequency,
    pub principal_frequency: PaymentFrequency,
    pub interest_accrued: Uint128,
    pub interest_owed: Uint128,
    pub last_update_ts: Timestamp,
}

impl CreditLine {
    fn total_interest_due(&self, env: &Env, interest_apr: u16) -> ContractResult<Uint128> {
        let period = env.block.time.seconds() - self.term_start.seconds();
        Ok(Uint128::zero())
    }

    fn principal_due_at(&self, env: &Env) -> ContractResult<Uint128> {
        Ok(Uint128::zero())
    }

    pub fn next_due_date(&self, env: &Env) -> ContractResult<Timestamp> {
        let _interest_due_date = self.next_interest_due_date(env)?;
        let _principal_due_date = self.next_principal_due_date(env)?;
        if _interest_due_date > _principal_due_date {
            return Ok(_interest_due_date);
        };
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

    pub fn interest_due(&self, env: &Env) -> ContractResult<Uint128> {
        if env.block.time < self.term_start {
            return Ok(Uint128::zero());
        }
        let past_interest = self.interest_accrued;
        let period = env.block.time.seconds() - self.last_update_ts.seconds();
        // (((borrow_amount / 10000) * interest) / 365*24*3600) * duration of borrow
        let latest_interest = self
            .borrow_info
            .borrowed_amount
            .checked_div(TEN_THOUSAND)?
            .checked_mul(Uint128::from(self.interest_apr))?
            .checked_div(SIY)?
            .checked_mul(Uint128::from(period))?;
        // !-------
        // calculate late fee
        // -------!

        Ok(past_interest + latest_interest)
    }

    pub fn principal_due(env: &Env) -> ContractResult<Uint128> {
        Ok(Uint128::zero())
    }
}
