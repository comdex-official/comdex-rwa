use cosmwasm_std::{
    to_json_binary, Addr, Binary, Decimal, Deps, Env, Order, QueryRequest, StdError, StdResult,
    Uint128, WasmQuery,
};
use cw721::{NftInfoResponse, OwnerOfResponse};
use cw721_metadata_onchain::{InvestorToken, QueryMsg as Cw721QueryMsg};

use crate::{
    contract::load_slices,
    error::ContractResult,
    msg::{AllPoolsResponse, PoolResponse, QueryMsg},
    state::{
        Config, RepaymentInfo, TranchePool, CONFIG, CREDIT_LINES, POOL_SLICES, REPAYMENTS,
        TRANCHE_POOLS, WHITELISTED_TOKENS,
    },
};

const DEFAULT_LIMIT: u8 = 50;
const MAX_LIMIT: u8 = 99;

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&get_config(deps, env)?),
        QueryMsg::GetPoolInfo { id } => to_json_binary(&get_pool_info(deps, env, id)?),
        QueryMsg::GetAllPools { start, limit } => {
            to_json_binary(&get_all_pools(deps, env, start, limit)?)
        }
    }
}

pub fn get_config(deps: Deps, _env: Env) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn get_pool_info(deps: Deps, _env: Env, id: u64) -> StdResult<TranchePool> {
    TRANCHE_POOLS.load(deps.storage, id)
}

pub fn get_all_pools(
    deps: Deps,
    _env: Env,
    start: Option<u64>,
    limit: Option<u8>,
) -> StdResult<AllPoolsResponse> {
    let start = start.unwrap_or(1u64);
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as u64;
    let end = start + limit;

    let mut pools = Vec::new();

    for pool_id in start..end {
        if !TRANCHE_POOLS.has(deps.storage, pool_id) {
            break;
        };
        let pool = TRANCHE_POOLS.load(deps.storage, pool_id)?;
        let credit_line = CREDIT_LINES.load(deps.storage, pool_id)?;
        let decimals = WHITELISTED_TOKENS.load(deps.storage, pool.denom.to_owned())?;
        pools.push(PoolResponse {
            pool_id,
            pool_name: pool.pool_name,
            borrower_name: pool.borrower_name,
            assets: credit_line.borrow_info.borrowed_amount,
            denom: pool.denom,
            decimals: decimals.1,
            apr: Decimal::from_atomics(credit_line.interest_apr as u128, 2)
                .map_err(|_| StdError::generic_err("interest apr conversion error"))?,
            pool_type: pool.pool_type,
            status: "OPEN".to_string(),
        });
    }

    Ok(AllPoolsResponse { data: pools })
}

pub fn get_nft_owner(deps: Deps, env: Env, token_id: u64) -> StdResult<Addr> {
    let config = get_config(deps.clone(), env)?;
    let query_msg = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.token_issuer.to_string(),
        msg: to_json_binary(&Cw721QueryMsg::OwnerOf {
            token_id: token_id.to_string(),
            include_expired: None,
        })?,
    });

    let result: OwnerOfResponse = deps.querier.query(&query_msg)?;
    Ok(deps.api.addr_validate(&result.owner)?)
}

pub fn get_nft_info(deps: Deps, env: Env, token_id: u64) -> StdResult<InvestorToken> {
    let config = get_config(deps.clone(), env.clone())?;
    let query_msg = Cw721QueryMsg::NftInfo {
        token_id: token_id.to_string(),
    };
    let nft_info: NftInfoResponse<InvestorToken> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.token_issuer.to_string(),
            msg: to_json_binary(&query_msg)?,
        }))?;
    Ok(nft_info.extension)
}

pub fn total_pool_deposit(deps: Deps, pool_id: u64) -> ContractResult<Uint128> {
    let slices = load_slices(deps, pool_id)?;
    let mut amount = Uint128::zero();

    for slice in slices.iter() {
        amount = amount.checked_add(slice.deposited()?)?;
    }

    Ok(amount)
}

pub fn get_repayment_info(deps: Deps, pool_id: u64) -> StdResult<Vec<RepaymentInfo>> {
    let payments = REPAYMENTS
        .may_load(deps.storage, pool_id)?
        .unwrap_or_default();
    Ok(payments)
}

pub fn pool_value_locked(deps: Deps, pool_id: u64) -> StdResult<Uint128> {
    let cl = CREDIT_LINES.load(deps.storage, pool_id)?;
    Ok(cl
        .borrow_info
        .total_borrowed
        .checked_sub(cl.borrow_info.principal_repaid)?)
}

pub fn total_value_locked(deps: Deps) -> StdResult<Uint128> {
    let mut locked_amount = Uint128::zero();
    let mut pool_value_locked: Uint128;
    for result in CREDIT_LINES.range(deps.storage, None, None, Order::Ascending) {
        if result.is_err() {
            continue;
        }
        let (pool_id, credit_line) = result.unwrap();
        pool_value_locked = credit_line
            .borrow_info
            .total_borrowed
            .checked_sub(credit_line.borrow_info.principal_repaid)
            .unwrap_or_default();
        locked_amount = locked_amount
            .checked_add(pool_value_locked)
            .unwrap_or_else(|_| locked_amount);
    }
    Ok(locked_amount)
}
