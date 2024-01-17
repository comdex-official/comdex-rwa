#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};

use crate::error::{ContractError, ContractResult};
use crate::msg::{CreatePoolMsg, ExecuteMsg, InstantiateMsg};
use crate::state::{Config, InvestorTotem, TranchePool, CONFIG, TRANCHE_POOLS};
use cw2::set_contract_version;
use cw721_base::ExecuteMsg as CW721ExecuteMsg;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    ensure_empty_funds(&info)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        pool_id: 0,
        grace_period: None,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    // Note: implement this function with different type to add support for custom messages
    // and then import the rest of this contract code.
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::NewPool { msg } => create_pool(deps, env, info, msg),
    }
}

pub fn create_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreatePoolMsg,
) -> ContractResult<Response> {
    // !-------
    // necessary validations
    // -------!
    // - verify sender
    // - verify all `msg` parameters
    ensure_empty_funds(&info)?;

    // create pool
    let mut config = CONFIG.load(deps.as_ref().storage)?;
    config.pool_id += 1;
    let borrower = deps.api.addr_validate(&msg.borrower)?;
    let tranche_pool = TranchePool::new(
        config.pool_id,
        msg.borrow_limit,
        msg.interest_apy,
        borrower,
        msg.payment_schedule,
        &env,
    );
    TRANCHE_POOLS.save(deps.storage, tranche_pool.pool_id, &tranche_pool)?;

    Ok(Response::new().add_attribute("method", "create_pool"))
}

fn ensure_empty_funds(info: &MessageInfo) -> ContractResult<()> {
    match info.funds.len() {
        0 => {}
        1 if info.funds[0].amount.is_zero() => {}
        _ => return Err(ContractError::FundsNotAllowed),
    }
    Ok(())
}
