#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Coin, CosmosMsg, DepsMut, Empty, Env, MessageInfo, Response, StdError, Uint128, WasmMsg,BankMsg,CustomQuery,QueryRequest,QueryResponse,from_binary,Addr,SubMsg,attr};

use cw2::set_contract_version;
use cw20::MinterResponse;
use cw20_base::{
    self,
    msg::{ExecuteMsg as Cw20ExecuteMsg, InstantiateMsg as Cw20InstantiateMsg},
};
use tranche_pool::{state::TranchePool, *};
use crate::error::ContractError;
use crate::msg::{DepositMsg, ExecuteMsg, InstantiateMsg};
use crate::state::{ADMIN, CONFIG, FUND_INFO};
use crate::ContractResult;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    if !info.funds.is_empty() {
        return Err(ContractError::FundsNotAllowed {});
    };

    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.set(deps, Some(admin))?;

    let init_msg = Cw20InstantiateMsg {
        name: "rLP Token".to_string(),
        symbol: "rLPT".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: env.contract.address.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let cosmos_msg = CosmosMsg::Wasm(WasmMsg::Instantiate {
        admin: Some(msg.admin.clone()),
        code_id: msg.cw20_code_id,
        msg: to_json_binary(&init_msg)?,
        funds: vec![],
        label: "LP Token #1".to_string(),
    });

    Ok(Response::default()
        .add_attribute("admin", msg.admin)
        .add_attribute("lp_code_id", msg.cw20_code_id.to_string())
        .add_message(cosmos_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        _ => unimplemented!(),
    }
}

pub fn deposit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: DepositMsg,
) -> ContractResult<Response> {
    match info.funds.len() {
        0 => return Err(ContractError::ZeroFunds {}),
        1 => {
            if info.funds[0].amount != msg.amount {
                return Err(ContractError::FundsMismatch {
                    required: msg.amount,
                    sent: info.funds[0].amount,
                });
            }
        }
        _ => return Err(ContractError::MultipleDenoms {}),
    }

    // epoch update

    let mut fund_info = FUND_INFO.load(deps.storage)?;
    fund_info.available += msg.amount;
    FUND_INFO.save(deps.storage, &fund_info)?;

    let config = CONFIG.load(deps.storage)?;
    let exn_msg = Cw20ExecuteMsg::Mint {
        recipient: info.sender.to_string(),
        amount: msg.amount,
    };
    let cosmos_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.lp_token,
        msg: to_json_binary(&exn_msg)?,
        funds: vec![],
    });

    Ok(Response::default()
        .add_attribute("method", "deposit")
        .add_attribute("amount", msg.amount.to_string())
        .add_attribute("user", info.sender.to_string())
        .add_message(cosmos_msg))
}

pub fn withdraw(deps: DepsMut, _env: Env, info: MessageInfo, amount: Uint128) -> ContractResult<Response> {
    //// check if amount is greater than 0
    if amount.is_zero() {
        return Err(StdError::generic_err("Withdraw must be greater than 0").into());
    }

    let mut fund_info = FUND_INFO.load(deps.storage)?;
    if amount > fund_info.available {
        return Err(StdError::generic_err("Insufficient funds").into());
    }

    //// CHECK THIS
    let withdraw_share=amount*fund_info.share_price;

    //// get user current share in token CW 20
    
    let current_share=Uint128::zero();

    if current_share<withdraw_share{
        return Err(StdError::generic_err("Amount requested is greater than what this address owns").into());
    }

    fund_info.available -= amount;
    FUND_INFO.save(deps.storage, &fund_info)?;

    let config = CONFIG.load(deps.storage)?;

    let mut messages=vec![];
    ///// send the amount to the user
    let bank_msg: CosmosMsg<Empty> = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.pool_denom,
            amount: amount,
        }],
    });

    messages.push(bank_msg);

    //// burn the share from the user
    let burn_msg = Cw20ExecuteMsg::Burn { amount: withdraw_share }; 

    let cosmos_msg: CosmosMsg<Empty> = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.lp_token,
        msg: to_json_binary(&burn_msg)?,
        funds: vec![],
    });

    messages.push(cosmos_msg.clone());

    Ok(Response::default()
        .add_attribute("method", "withdraw")
        .add_attribute("amount", amount.to_string())
        .add_attribute("user", info.sender.to_string())
        .add_messages(messages))
    
}

pub fn writedown(deps: DepsMut, _env: Env, _info: MessageInfo, token_id: Uint128) -> ContractResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut funds_info = FUND_INFO.load(deps.storage)?;
    let query_pool=tranche_pool::QueryMsg::GetPoolInfo{pool_id:pool_id};
    let query_msg: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.lp_token,
        msg: to_binary(&query_pool)?,
    });

    let pool: TranchePool = deps.querier.query(&query_msg)?;

    let mut token_info=pool.senior_tranche;

    ////// to do : access     // Assess the pool first in case it has unapplied USDC in its credit line

    let principal_remaining=token_info.principal_deposited-token_info.principal_redeemed;





    Ok(Response::default()
        .add_attribute("method", "writedown")
        .add_attribute("token_id", token_id.to_string()))
}

pub fn _calculate_writedown(principal_remaining:Uint128,pool:TranchePool)->ContractResult<(Uint128,Uint128)>{
    

    Ok(writedown)
}

pub fn invest(deps: DepsMut, _env: Env, _info: MessageInfo, pool_id: u64) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut funds_info = FUND_INFO.load(deps.storage)?;
    let query_pool=tranche_pool::QueryMsg::GetPoolInfo{pool_id:pool_id};
    let query_msg: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.lp_token,
        msg: to_binary(&query_pool)?,
    });

    let pool: TranchePool = deps.querier.query(&query_msg)?;

    //// Hardcoding an amount
    
    let amount=Uint128::from(1000000u128);

    if funds_info.available<amount{
        return Err(StdError::generic_err("not enough funds").into());
    }

    funds_info.available -= amount;

    funds_info.total_loans_outstanding += amount;

    FUND_INFO.save(deps.storage, &funds_info)?;

    let deposit_msg=tranche_pool::ExecuteMsg::Deposit{amount:amount,pool_id:pool_id};

    let deposit_msg: CosmosMsg<Empty> = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.lp_token,
        msg: to_binary(&deposit_msg)?,
        funds: vec![],
    });

    Ok(Response::default()
        .add_attribute("method", "invest")
        .add_attribute("pool_id", pool_id.to_string())
        .add_message(deposit_msg))
}



