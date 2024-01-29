#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Deps, DepsMut, Empty, Env, MessageInfo, Response, Uint128, WasmMsg,
};

use crate::credit_line::CreditLine;
use crate::error::{ContractError, ContractResult};
use crate::helpers::get_grace_period;
use crate::msg::{CreatePoolMsg, DepositMsg, DrawdownMsg, ExecuteMsg, InstantiateMsg};
use crate::state::{Config, InvestorToken, TranchePool, CONFIG, TRANCHE_POOLS, WHITELISTED_TOKENS};
use cw2::set_contract_version;
use cw721_base::{ExecuteMsg as CW721ExecuteMsg, MintMsg};

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

    let admins: Vec<Addr> = msg
        .admins
        .iter()
        .map_while(|val| deps.api.addr_validate(val).ok())
        .collect();
    if admins.len() < msg.admins.len() {
        return Err(ContractError::InvalidAdmin {
            address: msg.admins[admins.len()].to_owned(),
        });
    };

    let config = Config {
        pool_id: 0,
        admins,
        grace_period: None,
        token_id: 0,
        token_issuer: deps.api.addr_validate(&msg.token_issuer)?,
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
    let borrower = deps.api.addr_validate(&msg.borrower)?;

    let grace_period = get_grace_period(deps.as_ref())?;

    // create credit line
    let credit_line = CreditLine::new(
        msg.borrow_limit,
        msg.term_length,
        msg.drawdown_period,
        grace_period,
        msg.principal_grace_period,
        msg.interest_apr,
        msg.interest_payment_frequency,
        msg.principal_payment_frequency,
        &env,
    );

    // create pool
    let mut config = CONFIG.load(deps.as_ref().storage)?;
    config.pool_id += 1;
    let borrower = deps.api.addr_validate(&msg.borrower)?;
    let tranche_pool = TranchePool::new(
        config.pool_id,
        msg.borrow_limit,
        borrower.clone(),
        msg.drawdown_period,
        grace_period,
        credit_line,
        &env,
    );
    TRANCHE_POOLS.save(deps.storage, tranche_pool.pool_id, &tranche_pool)?;

    Ok(Response::new().add_attribute("method", "create_pool"))
}

pub fn deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: DepositMsg,
) -> ContractResult<Response> {
    match info.funds.len() {
        0 => return Err(ContractError::EmptyFunds),
        1 if info.funds[0].amount.is_zero() => return Err(ContractError::EmptyFunds),
        1 => {}
        _ => return Err(ContractError::MultipleTokens),
    };

    let iswhitelisted = WHITELISTED_TOKENS
        .may_load(deps.storage, info.funds[0].denom.clone())?
        .unwrap_or_default();
    if !iswhitelisted {
        return Err(ContractError::Unauthorized {});
    }

    if info.funds[0].amount != msg.amount {
        return Err(ContractError::FundDiscrepancy {
            required: msg.amount,
            sent: info.funds[0].amount,
        });
    }

    let mut pool = TRANCHE_POOLS
        .load(deps.storage, msg.pool_id)
        .map_err(|_| ContractError::InvalidPoolId { id: msg.pool_id })?;
    pool.deposit(msg.amount, &env)?;
    TRANCHE_POOLS.save(deps.storage, msg.pool_id, &pool)?;

    let mut config = CONFIG.load(deps.storage)?;
    config.token_id += 1;

    let mut nft = InvestorToken::new(config.token_id, msg.pool_id);
    nft.lend_info.principal_deposited += msg.amount;
    let mint_msg = CW721ExecuteMsg::<InvestorToken, Empty>::Mint(MintMsg {
        token_id: config.token_id.to_string(),
        owner: info.sender.to_string(),
        token_uri: None,
        extension: nft,
    });
    let msg = WasmMsg::Execute {
        contract_addr: config.token_issuer.to_string(),
        msg: to_json_binary(&mint_msg)?,
        funds: vec![],
    };

    Ok(Response::new().add_message(msg))
}

pub fn drawdown(deps: DepsMut, env: Env, msg: DrawdownMsg) -> ContractResult<Response> {
    // load pool info
    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
    // assert amount < available limit
    // assert msg.sender == borrower
    // assert no default
    // transfer amount to user
    Ok(Response::new())
}

pub fn load_pool(deps: Deps, pool_id: u64) -> ContractResult<TranchePool> {
    Ok(TRANCHE_POOLS
        .load(deps.storage, pool_id)
        .map_err(|_| ContractError::InvalidPoolId { id: pool_id })?)
}

pub fn whitelist_token(deps: DepsMut, denom: String) -> ContractResult<Response> {
    // !-------
    // Assert admin
    // -------!
    WHITELISTED_TOKENS.save(deps.storage, denom.clone(), &true)?;

    Ok(Response::new()
        .add_attribute("method", "whitelist_token")
        .add_attribute("new token", denom))
}

fn validate_create_pool_msg(msg: &CreatePoolMsg) -> ContractResult<()> {
    Ok(())
}

fn ensure_empty_funds(info: &MessageInfo) -> ContractResult<()> {
    match info.funds.len() {
        0 => {}
        1 if info.funds[0].amount.is_zero() => {}
        _ => return Err(ContractError::FundsNotAllowed),
    }
    Ok(())
}
