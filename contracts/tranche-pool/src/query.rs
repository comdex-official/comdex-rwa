use cosmwasm_std::{
    to_json_binary, Binary, Deps, Env, Order, QueryRequest, StdResult, Uint128, WasmQuery,
};
use cw721::NftInfoResponse;
use cw721_metadata_onchain::{InvestorToken, QueryMsg as Cw721QueryMsg};
use cw_storage_plus::Bound;

use crate::{
    msg::QueryMsg,
    state::{Config, TranchePool, CONFIG, TRANCHE_POOLS},
};

const DEFAULT_LIMIT: u8 = 10;
const MAX_LIMIT: u8 = 20;

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
) -> StdResult<Vec<TranchePool>> {
    let start = start.map(|s| Bound::inclusive(s));
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let pools: StdResult<Vec<TranchePool>> = TRANCHE_POOLS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| item.map(|(_, p)| p))
        .collect();

    Ok(pools?)
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
