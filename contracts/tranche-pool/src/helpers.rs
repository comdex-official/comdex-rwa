use cosmwasm_std::{
    to_json_binary, Addr, Decimal, Deps, DepsMut, Env, MessageInfo, Uint128, WasmQuery,
};

use crate::error::{ContractError, ContractResult};
use crate::state::{CreditLine, TrancheInfo, PAUSED};
use crate::{
    msg::CreatePoolMsg,
    state::{PoolSlice, CONFIG, KYC_CONTRACT, POOL_SLICES, WHITELISTED_TOKENS},
    GRACE_PERIOD,
};
use rwa_core::{
    msg::QueryMsg as CoreQuery,
    state::{ContactInfo, KYCStatus},
};

// 1e18
pub const SCALING_FACTOR: u128 = 1_000_000_000_000_000_000u128;

pub fn apply_to_all_slices(
    slices: &mut Vec<PoolSlice>,
    interest: Uint128,
    principal: Uint128,
    reserve_fee: u16,
    total_deployed: Uint128,
    junior_fee_percent: u16,
    credit_line: &CreditLine,
    env: &Env,
) -> ContractResult<Uint128> {
    // apply to senior tranches
    let reserve_amount = apply_to_senior_tranches(
        slices,
        interest,
        principal,
        reserve_fee,
        total_deployed,
        junior_fee_percent,
        credit_line,
        env,
    )?;
    // apply to junior tranches

    Ok(Uint128::zero())
}

pub fn apply_to_senior_tranches(
    slices: &mut Vec<PoolSlice>,
    interest: Uint128,
    principal: Uint128,
    reserve_fee_percent: u16,
    total_deployed: Uint128,
    junior_fee_percent: u16,
    credit_line: &CreditLine,
    env: &Env,
) -> ContractResult<Uint128> {
    let mut principal_owed = credit_line.principal_owed(env)?;
    principal_owed = principal_owed.checked_add(
        credit_line
            .borrow_info
            .total_borrowed
            .checked_sub(credit_line.borrow_info.borrowed_amount)?,
    )?;

    let mut senior_reserve_deduction = Uint128::zero();
    for slice in slices.iter_mut() {
        let mut principal_accrued = scale_for_slice(slice, principal_owed, total_deployed)?;
        let total_deposited = slice
            .senior_tranche
            .principal_deposited
            .checked_add(slice.junior_tranche.principal_deposited)?;
        principal_accrued = total_deposited
            .checked_sub(slice.principal_deployed)?
            .checked_add(principal_accrued)?;
        // apply to senior tranche
        let slice_reserve_deduction = slice.apply_to_senior_tranche(
            scale_for_slice(slice, interest, total_deployed)?,
            scale_for_slice(slice, principal, total_deployed)?,
            junior_fee_percent,
            reserve_fee_percent,
            principal_accrued,
        )?;
        senior_reserve_deduction = senior_reserve_deduction.checked_add(slice_reserve_deduction)?;
    }

    let junior_reserve_deduction = apply_to_junior_tranches(
        slices,
        interest,
        principal,
        junior_fee_percent,
        reserve_fee_percent,
        total_deployed,
        credit_line,
        env,
    )?;
    Ok(senior_reserve_deduction.checked_add(junior_reserve_deduction)?)
}

pub fn apply_to_junior_tranches(
    slices: &mut Vec<PoolSlice>,
    interest: Uint128,
    principal: Uint128,
    junior_fee_percent: u16,
    reserve_fee_percent: u16,
    total_deployed: Uint128,
    credit_line: &CreditLine,
    env: &Env,
) -> ContractResult<Uint128> {
    let mut princial_owed = credit_line.principal_owed(env)?;
    princial_owed = princial_owed.checked_add(
        credit_line
            .borrow_info
            .total_borrowed
            .checked_sub(credit_line.borrow_info.borrowed_amount)?,
    )?;
    let mut total_reserve_amount = Uint128::zero();
    for slice in slices.iter_mut() {
        let slice_reserve_deduction = slice.apply_to_junior_tranche(
            scale_for_slice(&slice, interest, total_deployed)?,
            scale_for_slice(&slice, principal, total_deployed)?,
            junior_fee_percent,
            reserve_fee_percent,
            princial_owed,
        )?;
        total_reserve_amount = total_reserve_amount.checked_add(slice_reserve_deduction)?;
    }
    Ok(total_reserve_amount)
}

pub fn scale_for_slice(
    slice: &PoolSlice,
    amount: Uint128,
    total_deployed: Uint128,
) -> ContractResult<Uint128> {
    Ok(scale_by_fraction(
        Decimal::from_atomics(amount, 0)?,
        slice.principal_deployed,
        total_deployed,
    )?
    .to_uint_floor())
}

pub fn apply_to_share_price(
    amount_remaining: Uint128,
    current_share_price: Decimal,
    mut desired_amount: Uint128,
    total_shares: Uint128,
) -> ContractResult<(Uint128, Decimal)> {
    if amount_remaining.is_zero() || desired_amount.is_zero() {
        return Ok((amount_remaining, current_share_price));
    }
    if amount_remaining < desired_amount {
        desired_amount = amount_remaining;
    }
    let share_price_diff = usdc_to_share_price(desired_amount, total_shares)?;
    Ok((
        amount_remaining.checked_sub(desired_amount)?,
        current_share_price.checked_add(share_price_diff)?,
    ))
}

pub fn desired_amount_from_share_price(
    mut desired_share_price: Decimal,
    actual_share_price: Decimal,
    total_shares: Uint128,
) -> ContractResult<Uint128> {
    if desired_share_price < actual_share_price {
        desired_share_price = actual_share_price;
    }

    let share_price_diff = desired_share_price.checked_sub(actual_share_price)?;
    share_price_to_usdc(share_price_diff, total_shares)
}

pub fn is_drawdown_paused(deps: Deps) -> ContractResult<bool> {
    Ok(PAUSED.may_load(deps.storage)?.unwrap_or_default())
}

pub fn ensure_drawdown_unpaused(deps: Deps) -> ContractResult<()> {
    if is_drawdown_paused(deps)? {
        return Err(ContractError::CustomError {
            msg: "Drawdowns have been paused".to_string(),
        });
    }
    Ok(())
}

pub fn get_tranche_info(
    tranche_id: u64,
    slices: &mut Vec<PoolSlice>,
) -> ContractResult<&mut TrancheInfo> {
    let slice_index = tranche_id as usize / 2;
    if slice_index >= slices.len() {
        return Err(ContractError::CustomError {
            msg: "tranche id exceeds range".to_string(),
        });
    }
    Ok(if tranche_id % 2 == 0 {
        &mut slices[slice_index].junior_tranche
    } else {
        &mut slices[slice_index].senior_tranche
    })
}

pub fn validate_create_pool_msg(
    deps: Deps,
    info: &MessageInfo,
    msg: &CreatePoolMsg,
) -> ContractResult<()> {
    ensure_whitelisted_denom(deps, msg.denom.clone())?;
    if msg.pool_name.is_empty() {
        return Err(ContractError::CustomError {
            msg: "Pool name cannot be empty".to_string(),
        });
    }
    if msg.borrower_name.is_empty() {
        return Err(ContractError::CustomError {
            msg: "Borrower name cannot be empty".to_string(),
        });
    }
    if msg.junior_fee_percent > 10000 {
        return Err(ContractError::CustomError {
            msg: "junior fee percent cannot be greater than 100%".to_string(),
        });
    }
    if msg.borrow_limit.is_zero() {
        return Err(ContractError::CustomError {
            msg: "Borrow limit should be non-zero".to_string(),
        });
    }
    if msg.term_length == 0 {
        return Err(ContractError::CustomError {
            msg: "Term length should be non-zero".to_string(),
        });
    }
    //if msg.term_length % msg.interest_frequency.to_seconds() != 0 {
        //return Err(ContractError::CustomError {
            //msg: "Term should be divisible by interest frequency".to_string(),
        //});
    //}
    //if msg.term_length % msg.principal_frequency.to_seconds() != 0 {
        //return Err(ContractError::CustomError {
            //msg: "Term should be divisible by principal frequency".to_string(),
        //});
    //}
    Ok(())
}

pub fn ensure_whitelisted_denom(deps: Deps, denom: String) -> ContractResult<()> {
    if !WHITELISTED_TOKENS
        .may_load(deps.storage, denom)?
        .unwrap_or_default()
    {
        return Err(ContractError::DenomNotWhitelisted);
    }
    Ok(())
}

pub fn ensure_empty_funds(info: &MessageInfo) -> ContractResult<()> {
    match info.funds.len() {
        0 => {}
        1 if info.funds[0].amount.is_zero() => {}
        _ => return Err(ContractError::FundsNotAllowed),
    }
    Ok(())
}

pub fn ensure_kyc(deps: Deps, user: Addr) -> ContractResult<()> {
    if !has_kyc(deps, user)? {
        return Err(ContractError::CustomError {
            msg: "non-KYC user".to_string(),
        });
    }
    Ok(())
}

pub fn has_kyc(deps: Deps, user: Addr) -> ContractResult<bool> {
    //Ok(KYC.may_load(deps.storage, user)?.unwrap_or_default())
    let kyc_contract = KYC_CONTRACT.load(deps.storage)?;
    let msg = to_json_binary(&CoreQuery::GetContactInfo { address: user })?;
    let wasm_msg = WasmQuery::Smart {
        contract_addr: kyc_contract.to_string(),
        msg,
    };
    let result = deps.querier.query::<ContactInfo>(&wasm_msg.into())?;
    match result.kyc_status {
        KYCStatus::Approved => Ok(true),
        _ => Ok(false),
    }
}

pub fn usdc_to_share_price(amount: Uint128, total_shares: Uint128) -> ContractResult<Decimal> {
    Ok(Decimal::new(
        amount
            .checked_mul(SCALING_FACTOR.into())?
            .checked_div(total_shares)?,
    ))
}

pub fn share_price_to_usdc(share_price: Decimal, total_shares: Uint128) -> ContractResult<Uint128> {
    Ok(share_price
        .checked_mul(Decimal::new(total_shares))?
        .to_uint_floor())
}

pub fn scale_by_fraction(
    share_price: Decimal,
    numerator: Uint128,
    denominator: Uint128,
) -> ContractResult<Decimal> {
    let numerator_decimal = Decimal::new(numerator);
    let denominator_decimal = Decimal::new(denominator);
    Ok(numerator_decimal
        .checked_div(denominator_decimal)?
        .checked_mul(share_price)?)
}

pub fn initialize_next_slice(deps: DepsMut, pool_id: u64) -> ContractResult<()> {
    let updated_slices = match POOL_SLICES.may_load(deps.storage, pool_id)? {
        Some(mut slices) => {
            if slices.len() >= 5 {
                return Err(ContractError::MaxSliceLimit);
            };
            if slices[slices.len() - 1].is_locked() {
                return Err(ContractError::CustomError {
                    msg: "All previous slices should be locked".to_string(),
                });
            }
            // !-------
            // Check for late payment
            // -------!
            // !-------
            // Should be within principal grace period
            // -------!
            slices.push(PoolSlice::new(slices.len() as u64)?);
            slices
        }
        None => {
            vec![PoolSlice::new(0u64)?]
        }
    };
    Ok(())
}

pub fn get_grace_period(deps: Deps) -> ContractResult<u64> {
    let config = CONFIG.load(deps.storage)?;

    Ok(config.grace_period.unwrap_or(GRACE_PERIOD))
}
