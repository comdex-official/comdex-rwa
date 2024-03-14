#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{attr, from_binary, to_binary, to_json_binary, Addr,Deps, BankMsg, Coin, CosmosMsg, CustomQuery, DepsMut, Empty, Env, MessageInfo, QueryRequest, QueryResponse, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg, WasmQuery,Decimal,Binary};

use cw2::set_contract_version;
use cw20::MinterResponse;
use cw20_base::{
    self,
    msg::{ExecuteMsg as Cw20ExecuteMsg, InstantiateMsg as Cw20InstantiateMsg},
};
use tranche_pool::*;
use tranche_pool::msg::QueryMsg as TranchePoolQueryMsg;
use tranche_pool::state::*;
use crate::{error::ContractError, state::WRITE_DOWNS_BY_POOL_AMOUNT};
use crate::msg::{DepositMsg, ExecuteMsg, InstantiateMsg,QueryMsg};
use crate::state::{ADMIN, CONFIG, FUND_INFO,EPOCH,CHECK_POINTED_EPOCH_ID,EPOCH_DURATION,Epoch,USER_DEPOSIT,Deposits};
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

    //// initialize epoch /////
    let epoch=Epoch{
        end_time:env.block.time,
        lp_token_requested:Uint128::zero(),
        lp_token_liquidated:Uint128::zero(),
        usdc_allocated:Uint128::zero(),
    };

    EPOCH.save(deps.storage, 0,&epoch)?;
    
    EPOCH_DURATION.save(deps.storage, &21600)?;

    ADMIN.set(deps, Some(admin))?;
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
        ExecuteMsg::Deposit(msg) => deposit(_deps, _env, _info, msg),
        ExecuteMsg::Withdraw { amount } => withdraw(_deps, _env, _info, amount),
        ExecuteMsg::Invest { pool_id ,tranche_id,amount} => invest(_deps, _env, _info, pool_id,tranche_id,amount),
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

    let deposit=USER_DEPOSIT.may_load(deps.storage, info.clone().sender)?;
    let latest_epoch=CHECK_POINTED_EPOCH_ID.load(deps.storage)?;
    if deposit.is_none(){
        let deposit=Deposits{
            deposited:msg.amount,
            deposit_epoch:latest_epoch,
        };
        USER_DEPOSIT.save(deps.storage, info.clone().sender, &deposit)?;
    }
    else{
        let mut deposit=deposit.unwrap();
        deposit.deposited+=msg.amount;
        deposit.deposit_epoch=latest_epoch;
        USER_DEPOSIT.save(deps.storage, info.clone().sender, &deposit)?;
    }

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

    let deposit=USER_DEPOSIT.may_load(deps.storage, info.clone().sender)?;

    if deposit.is_none(){
        return Err(StdError::generic_err("No deposit found").into());
    }

    let mut deposit=deposit.unwrap();

    if deposit.deposited<amount{
        return Err(StdError::generic_err("Amount requested is greater than what this address owns").into());
    }

    deposit.deposited-=amount;

    USER_DEPOSIT.save(deps.storage, info.clone().sender, &deposit)?;

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

pub fn writedown(deps: DepsMut, _env: Env, _info: MessageInfo, token_id: u64) -> ContractResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut funds_info = FUND_INFO.load(deps.storage)?;
    let query_pool=tranche_pool::msg::QueryMsg::GetPoolInfo{id:token_id};
    let query_msg: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.lp_token,
        msg: to_binary(&query_pool)?,
    });

    let pool: tranche_pool::state::TranchePool = deps.querier.query(&query_msg)?;

    let mut token_info=pool.senior_tranche.clone();

    ////// to do : access     // Assess the pool first in case it has unapplied USDC in its credit line

    let principal_remaining=token_info.principal_deposited-token_info.principal_redeemed;

    let (write_down_percent,write_down_amount)=_calculate_writedown(principal_remaining,pool)?;

    let previous_writedown=WRITE_DOWNS_BY_POOL_AMOUNT.load(deps.storage, token_id)?;

    if write_down_percent==Uint128::zero() && previous_writedown==Uint128::zero(){
        return Err(StdError::generic_err("No writedown required").into());
    }

    let write_down_delta=write_down_amount-previous_writedown;
    WRITE_DOWNS_BY_POOL_AMOUNT.save(deps.storage, token_id, &write_down_amount)?;

    if write_down_delta>Uint128::zero(){

        funds_info.total_writedowns-=write_down_delta;
    }
    else{
        funds_info.total_writedowns+=write_down_delta;
    }

    Ok(Response::default()
        .add_attribute("method", "writedown")
        .add_attribute("token_id", token_id.to_string()))
}

pub fn _calculate_writedown(principal_remaining:Uint128,pool:tranche_pool::state::TranchePool)->ContractResult<(Uint128,Uint128)>{

    Ok((Uint128::zero(),Uint128::zero()))

}

pub fn invest(deps: DepsMut, _env: Env, _info: MessageInfo, pool_id: u64,tranche_id:u64,amount:Uint128) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut funds_info = FUND_INFO.load(deps.storage)?;
    let query_pool=tranche_pool::msg::QueryMsg::GetPoolInfo{id:pool_id};
    let query_msg: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.lp_token.clone(),
        msg: to_binary(&query_pool)?,
    });

    let pool:tranche_pool::state::TranchePool = deps.querier.query(&query_msg)?;

    //// Hardcoding an amount
    
    let amount=Uint128::from(1000000u128);

    if funds_info.available<amount{
        return Err(StdError::generic_err("not enough funds").into());
    }

    funds_info.available -= amount;

    funds_info.total_loans_outstanding += amount;

    FUND_INFO.save(deps.storage, &funds_info)?;

    let deposit_msg=tranche_pool::msg::ExecuteMsg::Deposit{msg:tranche_pool::msg::DepositMsg{amount:amount,pool_id:pool_id}};

    let deposit_msg: CosmosMsg<Empty> = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.clone().lp_token.clone(),
        msg: to_binary(&deposit_msg)?,
        funds: vec![],
    });

    Ok(Response::default()
        .add_attribute("method", "invest")
        .add_attribute("pool_id", pool_id.to_string())
        .add_message(deposit_msg))
}

pub fn _apply_epoch_checkpoints(deps: DepsMut, _env: Env, _info: MessageInfo) -> ContractResult<Response> {
    let mut epoch_id = CHECK_POINTED_EPOCH_ID.load(deps.storage)?;
    let mut epoch = EPOCH.load(deps.storage, epoch_id)?;

    if epoch.end_time < _env.block.time {
        let mut funds_info = FUND_INFO.load(deps.storage)?;
        funds_info.available += epoch.lp_token_liquidated;
        funds_info.total_loans_outstanding -= epoch.usdc_allocated;
        FUND_INFO.save(deps.storage, &funds_info)?;

        epoch_id += 1;
        CHECK_POINTED_EPOCH_ID.save(deps.storage, &epoch_id)?;
        epoch = EPOCH.load(deps.storage, epoch_id)?;
    }
    Ok(Response::new())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetUserDeposits { address } => to_binary(&query_user_deposits(deps, address)?),
        QueryMsg::GetConfig {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetFundInfo {} => to_binary(&FUND_INFO.load(deps.storage)?),
        QueryMsg::MaxLeverageRatio {} => to_binary(&get_leverage_ratio(deps)?),
    }
}
 

pub fn query_user_deposits(deps: Deps, address: Addr) -> StdResult<Deposits> {
    let deposit=USER_DEPOSIT.may_load(deps.storage, address)?;
    if deposit.is_none(){
        return Err(StdError::generic_err("No deposit found").into());
    }
    Ok(deposit.unwrap())
}

pub fn get_leverage_ratio(deps: Deps) -> StdResult<Decimal> {
    let config=CONFIG.load(deps.storage)?;
    return Ok(config.max_leverage_ratio);
}
