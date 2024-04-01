use cosmwasm_std::{Decimal, Env, Timestamp, Uint128};
use cw721_metadata_onchain::InvestorToken;

use crate::{
    error::ContractResult,
    helpers::{
        apply_to_share_price, desired_amount_from_share_price, scale_by_fraction,
        share_price_to_usdc, usdc_to_share_price,
    },
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

    pub fn deposited(&self) -> ContractResult<Uint128> {
        Ok(self
            .junior_tranche
            .principal_deposited
            .checked_add(self.senior_tranche.principal_deposited)?)
    }

    pub fn is_locked(&self) -> bool {
        self.senior_tranche.locked_until != Timestamp::default()
    }

    pub fn apply_to_senior_tranche(
        &mut self,
        interest_remaining: Uint128,
        principal_remaining: Uint128,
        junior_fee_percent: u16,
        reserve_fee_percent: u16,
        principal_accrued: Uint128,
    ) -> ContractResult<Uint128> {
        let expected_interest_share_price = self
            .senior_tranche
            .expected_share_price(self.total_interest_accrued, &self)?;
        let expected_principal_share_price = self
            .senior_tranche
            .expected_share_price(principal_accrued, &self)?;
        let desired_net_interest_share_price = scale_by_fraction(
            expected_interest_share_price,
            Uint128::new(10000u128).checked_sub(Uint128::new(
                (junior_fee_percent + reserve_fee_percent) as u128,
            ))?,
            Uint128::new(10000u128),
        )?;
        let reserve_deduction = scale_by_fraction(
            Decimal::from_atomics(interest_remaining, 0)?,
            Uint128::new(reserve_fee_percent as u128),
            Uint128::new(10000u128),
        )?;
        Ok(Uint128::zero())
    }

    pub fn apply_to_junior_tranche(
        &mut self,
        interest_remaining: Uint128,
        principal_remaining: Uint128,
        junior_fee_percent: u16,
        reserve_fee_percent: u16,
        principal_accrued: Uint128,
    ) -> ContractResult<Uint128> {
        let expected_interest_share_price =
            self.junior_tranche
                .interest_share_price
                .checked_add(usdc_to_share_price(
                    interest_remaining,
                    self.junior_tranche.principal_deposited,
                )?)?;
        let expected_principal_share_price = self
            .junior_tranche
            .expected_share_price(principal_accrued, &self)?;

        let old_interest_share_price = self.junior_tranche.interest_share_price;
        let old_principal_share_price = self.junior_tranche.principal_share_price;

        let (mut interest_remaining, mut principal_remaining) =
            self.junior_tranche.apply_by_share_price(
                interest_remaining,
                principal_remaining,
                expected_interest_share_price,
                expected_principal_share_price,
            )?;

        interest_remaining = interest_remaining.checked_add(principal_remaining)?;

        let reserve_deduction = scale_by_fraction(
            Decimal::from_atomics(principal_remaining, 0)?,
            Uint128::new(reserve_fee_percent as u128),
            Uint128::new(10000u128),
        )?
        .to_uint_floor();

        interest_remaining = interest_remaining.checked_sub(reserve_deduction)?;
        principal_remaining = Uint128::zero();

        (interest_remaining, principal_remaining) = self.junior_tranche.apply_by_amount(
            interest_remaining.checked_add(principal_remaining)?,
            Uint128::zero(),
            interest_remaining.checked_add(principal_remaining)?,
            Uint128::zero(),
        )?;
        Ok(reserve_deduction)
    }
}

impl TrancheInfo {
    pub fn redeemable_interest_and_amount(
        &self,
        investor_token: &InvestorToken,
    ) -> ContractResult<(Uint128, Uint128)> {
        let max_principal_redeemable = share_price_to_usdc(
            self.principal_share_price,
            investor_token.lend_info.principal_deposited,
        )?;
        let max_interest_redeemable = share_price_to_usdc(
            self.interest_share_price,
            investor_token.lend_info.principal_deposited,
        )?;
        let redeemable_principal =
            max_principal_redeemable.checked_sub(investor_token.lend_info.principal_redeemed)?;
        let redeemable_interest =
            max_interest_redeemable.checked_sub(investor_token.lend_info.interest_redeemed)?;
        Ok((redeemable_interest, redeemable_principal))
    }

    pub fn is_senior_tranche(&self) -> bool {
        if self.id % 2 == 1 {
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
        if self.principal_deposited.is_zero() {
            return Ok(Decimal::zero())
        };
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

    pub fn apply_by_share_price(
        &mut self,
        interest_remaining: Uint128,
        principal_remaining: Uint128,
        desired_interest_share_price: Decimal,
        desired_principal_share_price: Decimal,
    ) -> ContractResult<(Uint128, Uint128)> {
        let desired_interest_amount = desired_amount_from_share_price(
            desired_interest_share_price,
            self.interest_share_price,
            self.principal_deposited,
        )?;
        let desired_principal_amount = desired_amount_from_share_price(
            desired_principal_share_price,
            self.principal_share_price,
            self.principal_deposited,
        )?;

        self.apply_by_amount(
            interest_remaining,
            principal_remaining,
            desired_interest_amount,
            desired_principal_amount,
        )
    }

    pub fn apply_by_amount(
        &mut self,
        interest_remaining: Uint128,
        principal_remaining: Uint128,
        desired_interest_amount: Uint128,
        desired_principal_amount: Uint128,
    ) -> ContractResult<(Uint128, Uint128)> {
        let total_shares = self.principal_deposited;

        let (interest_remaining, new_share_price) = apply_to_share_price(
            interest_remaining,
            self.interest_share_price,
            desired_interest_amount,
            total_shares,
        )?;
        self.interest_share_price = new_share_price;

        let (principal_remaining, new_share_price) = apply_to_share_price(
            principal_remaining,
            self.principal_share_price,
            desired_principal_amount,
            total_shares,
        )?;
        self.principal_share_price = new_share_price;
        Ok((interest_remaining, principal_remaining))
    }
}
