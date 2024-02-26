use cosmwasm_std::{Decimal, Env, Timestamp, Uint128};

use crate::{
    error::ContractResult,
    helpers::{usdc_to_share_price, scale_by_fraction},
    state::{PoolSlice, TrancheInfo},
};

impl PoolSlice {
    pub fn new(slice_index: u64) -> ContractResult<PoolSlice> {
        Ok(PoolSlice {
            junior_tranche: TrancheInfo {
                id: slice_index * 2,
                principal_deposited: Uint128::zero(),
                locked_until: Timestamp::default(),
                principal_share_price: usdc_to_share_price(1u128.into(), 1u128.into())?,
                interest_share_price: Decimal::zero(),
            },
            senior_tranche: TrancheInfo {
                id: slice_index * 2 + 1,
                principal_deposited: Uint128::zero(),
                locked_until: Timestamp::default(),
                principal_share_price: usdc_to_share_price(1u128.into(), 1u128.into())?,
                interest_share_price: Decimal::zero(),
            },
            total_interest_accrued: 0u128.into(),
            principal_deployed: 0u128.into(),
        })
    }

    pub fn is_locked(&self) -> bool {
        self.senior_tranche.locked_until != Timestamp::default()
    }
}

impl TrancheInfo {
    pub fn is_senior_tranche(&self) -> bool {
        if self.id % 2 == 0 {
            true
        } else {
            false
        }
    }

    pub fn lock_tranche(&mut self, env: &Env, drawdown_period: u64) -> ContractResult<()> {
        self.locked_until = env.block.time.plus_seconds(drawdown_period);
        Ok(())
    }

    pub fn expected_share_price(
        &self,
        amount: Uint128,
        slice: &PoolSlice,
    ) -> ContractResult<Decimal> {
        let share_price = usdc_to_share_price(amount, self.principal_deposited)?;
        self.scale_by_percent_ownership(share_price, slice)
    }

    pub fn scale_by_percent_ownership(
        &self,
        share_price: Decimal,
        slice: &PoolSlice,
    ) -> ContractResult<Decimal> {
        let total_deposited = slice
            .junior_tranche
            .principal_deposited
            .checked_add(slice.senior_tranche.principal_deposited)?;
        scale_by_fraction(share_price, self.principal_deposited, total_deposited)
    }
}
