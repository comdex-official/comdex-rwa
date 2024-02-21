use cosmwasm_std::{Decimal, Timestamp, Uint128};

use crate::{
    error::ContractResult,
    helpers::usdc_to_share_price,
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
}
