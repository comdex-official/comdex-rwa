use cosmwasm_std::{to_json_binary, Binary, Deps, Env, StdError, StdResult};

use crate::{
    msg::QueryMsg,
    state::{Config, TranchePool, CONFIG, KYC, TRANCHE_POOLS},
};

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&get_config(deps, env)?),
        QueryMsg::GetPoolInfo { id } => to_json_binary(&get_pool_info(deps, env, id)?),
        QueryMsg::CheckKycStatus { user } => to_json_binary(&check_kyc_status(deps, env, user)?),
    }
}

pub fn get_config(deps: Deps, _env: Env) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn get_pool_info(deps: Deps, _env: Env, id: u64) -> StdResult<TranchePool> {
    TRANCHE_POOLS.load(deps.storage, id)
}

pub fn check_kyc_status(deps: Deps, _env: Env, user: String) -> StdResult<bool> {
    Ok(KYC
        .load(deps.storage, deps.api.addr_validate(&user)?)
        .unwrap_or(false))
}
