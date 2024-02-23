use cosmwasm_std::{to_json_binary, Addr, Decimal, Deps, DepsMut, MessageInfo, Uint128, WasmQuery};

use crate::error::{ContractError, ContractResult};
use crate::state::TrancheInfo;
use crate::{
    msg::CreatePoolMsg,
    state::{PoolSlice, CONFIG, KYC_CONTRACT, POOL_SLICES, WHITELISTED_TOKENS},
    GRACE_PERIOD,
};
use rwa_core::{
    msg::QueryMsg as CoreQuery,
    state::{ContactInfo, KYCStatus},
};

pub fn get_tranche_info<'a>(
    tranche_id: u64,
    slices: &'a mut Vec<PoolSlice>,
) -> ContractResult<&'a mut TrancheInfo> {
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
        amount.checked_div(total_shares).unwrap_or_default(),
    ))
}

pub fn share_price_to_usdc(share_price: Decimal, total_shares: Uint128) -> ContractResult<Uint128> {
    Ok(share_price
        .checked_mul(Decimal::new(total_shares))?
        .to_uint_floor())
}

pub fn initialize_next_slice(deps: DepsMut, pool_id: u64) -> ContractResult<()> {
    let updated_slices = match POOL_SLICES.may_load(deps.storage, pool_id)? {
        Some(mut slices) => {
            if slices.len() >= 5 {
                return Err(ContractError::MaxSliceLimit);
            };
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
