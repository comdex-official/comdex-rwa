use cosmwasm_std::{
    to_json_binary, Addr, Binary, Decimal, Deps, Env, Order, QueryRequest, StdError, StdResult,
    Timestamp, Uint128, WasmQuery,
};
use cw721::{NftInfoResponse, OwnerOfResponse, TokensResponse};
use cw721_metadata_onchain::{InvestorToken, QueryMsg as Cw721QueryMsg};
use rwa_core;

use crate::{
    contract::load_slices,
    error::ContractResult,
    msg::{AllPoolsResponse, PoolInfo, PoolResponse, QueryMsg},
    state::{
        Config, CreditLine, PoolSlice, RepaymentInfo, CONFIG, CREDIT_LINES, PAY_CONTRACT,
        POOL_SLICES, REPAYMENTS, TRANCHE_POOLS, WHITELISTED_TOKENS,
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
        QueryMsg::RepaymentInfo { id } => to_json_binary(&get_repayment_info(deps, id)?),
        QueryMsg::GetCreditLine { id } => to_json_binary(&get_credit_line(deps, env, id)?),
        QueryMsg::GetSlices { id } => to_json_binary(&get_slices(deps, env, id)?),
        QueryMsg::GetInvestmentInfo {token_id} => to_json_binary(&get_nft_info(deps, env, token_id)?)
    }
}

pub fn get_config(deps: Deps, _env: Env) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn get_pool_info(deps: Deps, env: Env, id: u64) -> StdResult<Option<PoolInfo>> {
    if !TRANCHE_POOLS.has(deps.storage, id) {
        return Ok(None);
    };
    let pool = TRANCHE_POOLS.load(deps.storage, id)?;
    let cl = CREDIT_LINES.load(deps.storage, id)?;
    let slices = POOL_SLICES.load(deps.storage, id)?;

    let invested = slices.iter().fold(Uint128::zero(), |acc, slice| {
        acc.checked_add(slice.deposited().unwrap_or_default())
            .unwrap_or_default()
    });
    let mut junior_capital_locked = false;
    let mut pool_locked = false;
    let mut amount_available = Uint128::zero();
    if let Some(slice) = slices.last() {
        if slice.junior_tranche.locked_until != Timestamp::default() {
            junior_capital_locked = true;
        }
        if slice.is_locked() && slice.senior_tranche.locked_until > env.block.time {
            pool_locked = true;
            amount_available = slice
                .deposited()
                .unwrap_or_default()
                .saturating_sub(slice.principal_deployed);
        }
    };
    let mut _env = env.clone();
    _env.block.time = cl.prev_due_date(&env).unwrap_or_default();
    let interest_pending = cl.interest_owed(&_env).unwrap_or_default();

    let tranche_id: String = if slices.is_empty() || slices.last().unwrap().is_locked() {
        "".to_string()
    } else {
        slices.last().unwrap().junior_tranche.id.to_string()
    };

    let pay_contract = PAY_CONTRACT.load(deps.storage)?;
    let query_msg = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pay_contract.to_string(),
        msg: to_json_binary(&rwa_core::msg::QueryMsg::GetConfig {})?,
    });

    let rwa_config = deps.querier.query::<rwa_core::state::Config>(&query_msg)?;

    let mut asset_info: Option<rwa_core::state::Asset> = None;
    for asset in rwa_config.accepted_assets.iter() {
        if asset.denom == pool.denom {
            asset_info = Some(asset.to_owned());
            break;
        }
    }

    Ok(Some(PoolInfo {
        pool_id: id,
        pool_name: pool.pool_name,
        borrower_name: pool.borrower_name,
        borrower: pool.borrower_addr.to_string(),
        assets: cl.borrow_info.borrowed_amount,
        asset_info,
        apr: Decimal::from_atomics(cl.interest_apr as u128, 2).unwrap_or_default(),
        pool_type: pool.pool_type,
        status: String::from("Open"),
        invested,
        drawn: cl.borrow_info.total_borrowed,
        available_to_draw: amount_available,
        interest_paid: cl.borrow_info.interest_repaid,
        interest_accrued: cl.interest_accrued(&env).unwrap_or_default(),
        interest_pending,
        tranche_id,
        borrow_limit: cl.borrow_info.borrow_limit,
        junior_capital_locked,
        pool_locked,
    }))
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
        let token = WHITELISTED_TOKENS.load(deps.storage, pool.denom.to_owned())?;
        pools.push(PoolResponse {
            pool_id,
            pool_name: pool.pool_name,
            borrower_name: pool.borrower_name,
            borrower: pool.borrower_addr.to_string(),
            assets: credit_line.borrow_info.borrowed_amount,
            denom: pool.denom,
            decimals: token.1,
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

pub fn repayment_info(deps: Deps, env: Env, id: u64) -> StdResult<Vec<RepaymentInfo>> {
    REPAYMENTS.load(deps.storage, id)
}

pub fn get_credit_line(deps: Deps, env: Env, id: u64) -> StdResult<CreditLine> {
    CREDIT_LINES.load(deps.storage, id)
}

pub fn get_slices(deps: Deps, env: Env, id: u64) -> StdResult<Vec<PoolSlice>> {
    POOL_SLICES.load(deps.storage, id)
}

pub fn get_investments(
    deps: Deps,
    env: Env,
    user: String,
    pool_id: u64,
) -> StdResult<Vec<InvestorToken>> {
    let query_msg = Cw721QueryMsg::Tokens {
        owner: user,
        start_after: None,
        limit: None,
    };
    let config = get_config(deps.clone(), env.clone())?;
    let result = deps
        .querier
        .query::<TokensResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.token_issuer.to_string(),
            msg: to_json_binary(&query_msg)?,
        }))?;
    let mut investments = Vec::new();
    for token_id in result.tokens.iter() {
        let token_info = get_nft_info(
            deps.clone(),
            env.clone(),
            token_id.parse::<u64>().map_err(|_| {
                StdError::generic_err(format!("Unable to parse {token_id} into integer"))
            })?,
        )?;
        if token_info.pool_id == pool_id {
            investments.push(token_info);
        }
    }
    Ok(investments)
}
