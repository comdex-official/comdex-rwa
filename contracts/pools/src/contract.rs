#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_json_binary, Addr, BankMsg, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, Uint128, WasmMsg,
};

use crate::credit_line::CreditLine;
use crate::error::{ContractError, ContractResult};
use crate::helpers::get_grace_period;
use crate::msg::{
    CreatePoolMsg, DepositMsg, DrawdownMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, RepayMsg,
};
use crate::state::{
    Config, InvestorToken, TranchePool, CONFIG, KYC, TRANCHE_POOLS, USDC, WHITELISTED_TOKENS,
};
use cw2::set_contract_version;
use cw721_base::{ExecuteMsg as CW721ExecuteMsg, InstantiateMsg as Cw721IntantiateMsg, MintMsg};

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
    USDC.save(deps.storage, &msg.usdc_denom)?;

    let cw721_msg = Cw721IntantiateMsg {
        name: "PoolToken".to_string(),
        symbol: "PoTo".to_string(),
        collection_uri: None,
        minter: env.contract.address.to_string(),
        metadata: Empty {},
    };
    let cosmos_msg = CosmosMsg::Wasm(WasmMsg::Instantiate {
        admin: Some(info.sender.to_string()),
        code_id: msg.code_id,
        msg: to_json_binary(&cw721_msg)?,
        funds: vec![],
        label: "Pool Token".to_string(),
    });

    Ok(Response::default().add_message(cosmos_msg))
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
        ExecuteMsg::UpdateKyc { user, kyc_status } => {
            update_user_kyc(deps, env, info, user, kyc_status)
        }
        ExecuteMsg::Deposit { msg } => deposit(deps, env, info, msg),
        ExecuteMsg::Repay { msg } => repay(deps, env, info, msg),
        ExecuteMsg::Drawdown { msg } => drawdown(deps, env, info, msg),
    }
}

pub fn update_user_kyc(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user: Addr,
    kyc_status: bool,
) -> ContractResult<Response> {
    KYC.save(deps.storage, user.clone(), &kyc_status)?;

    Ok(Response::new().add_attribute(user.to_string(), kyc_status.to_string()))
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
    if info.sender != borrower {
        return Err(ContractError::Unauthorized {});
    };
    if !has_kyc(deps.as_ref(), borrower.clone())? {
        return Err(ContractError::CustomError {
            msg: "non-KYC user".to_string(),
        });
    }

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

    Ok(Response::new()
        .add_attribute("method", "create_pool")
        .add_attribute("pool_id", config.pool_id.to_string()))
}

pub fn deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: DepositMsg,
) -> ContractResult<Response> {
    // !-------
    // the addr of user should be in the backer list to allow deposit
    // -------!
    if !has_kyc(deps.as_ref(), info.sender.clone())? {
        return Err(ContractError::CustomError {
            msg: "non-KYC user".to_string(),
        });
    }
    match info.funds.len() {
        0 => return Err(ContractError::EmptyFunds),
        1 if info.funds[0].amount.is_zero() => return Err(ContractError::EmptyFunds),
        1 => {}
        _ => return Err(ContractError::MultipleTokens),
    };
    let usdc_denom = USDC.load(deps.storage)?;
    if info.funds[0].denom != usdc_denom {
        return Err(ContractError::CustomError {
            msg: "Not USDC".to_string(),
        });
    }

    //let iswhitelisted = WHITELISTED_TOKENS
    //.may_load(deps.storage, info.funds[0].denom.clone())?
    //.unwrap_or_default();
    //if !iswhitelisted {
    //return Err(ContractError::Unauthorized {});
    //}

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

pub fn drawdown(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: DrawdownMsg,
) -> ContractResult<Response> {
    if !has_kyc(deps.as_ref(), info.sender.clone())? {
        return Err(ContractError::CustomError {
            msg: "non-KYC user".to_string(),
        });
    }
    // load pool info
    let mut pool = load_pool(deps.as_ref(), msg.pool_id)?;
    // assert amount < available limit
    // assert msg.sender == borrower
    // assert no default
    if info.sender != pool.borrower_addr {
        return Err(ContractError::Unauthorized {});
    }
    pool.drawdown(msg.amount, &env)?;
    // transfer amount to user
    let usdc_denom = USDC.load(deps.storage)?;
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(msg.amount.u128(), usdc_denom),
    });
    Ok(Response::new().add_message(msg))
}

pub fn repay(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut msg: RepayMsg,
) -> ContractResult<Response> {
    if !has_kyc(deps.as_ref(), info.sender.clone())? {
        return Err(ContractError::CustomError {
            msg: "non-KYC user".to_string(),
        });
    }
    if info.funds.is_empty() {
        return Err(ContractError::EmptyFunds);
    } else if info.funds.len() > 1 {
        return Err(ContractError::MultipleTokens);
    }
    if info.funds[0].amount != msg.amount {
        return Err(ContractError::FundDiscrepancy {
            required: msg.amount,
            sent: info.funds[0].amount,
        });
    }
    let usdc_denom = USDC.load(deps.storage)?;
    if info.funds[0].denom != usdc_denom {
        return Err(ContractError::CustomError {
            msg: "Not USDC".to_string(),
        });
    }
    let mut pool = load_pool(deps.as_ref(), msg.pool_id)?;
    let pending_payments = pool.repay(&mut msg.amount, &env)?;

    // !-------
    // Handle pending_payments
    // -------!

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

pub fn has_kyc(deps: Deps, user: Addr) -> ContractResult<bool> {
    Ok(KYC.may_load(deps.storage, user)?.unwrap_or(false))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult<Response> {
    let ver = cw2::get_contract_version(deps.storage)?;
    // ensure we are migrating from an allowed contract
    if ver.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type").into());
    }
    // note: better to do proper semver compare, but string compare *usually* works
    if ver.version.as_str() > CONTRACT_VERSION {
        return Err(StdError::generic_err("Cannot upgrade from a newer version").into());
    }
    // set the new version
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}
