// TODO
// - add txns to change the backer list
// - add txn to change the senior-pool contract addr
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_json_binary, Addr, BankMsg, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, Uint128, WasmMsg, WasmQuery,
};

use cw2::set_contract_version;
use cw721_base::{ExecuteMsg as CW721ExecuteMsg, InstantiateMsg as Cw721IntantiateMsg, MintMsg};

use crate::{
    error::{ContractError, ContractResult},
    helpers::{
        ensure_empty_funds, ensure_kyc, ensure_whitelisted_denom, get_grace_period,
        get_tranche_info, initialize_next_slice, is_backer, validate_create_pool_msg,
    },
    msg::{
        CreatePoolMsg, DepositMsg, DrawdownMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, RepayMsg,
    },
    state::{
        Config, CreditLine, InvestorToken, PoolSlice, TranchePool, CONFIG, CREDIT_LINES, KYC,
        KYC_CONTRACT, POOL_SLICES, SENIOR_POOLS, TRANCHE_POOLS, USDC, WHITELISTED_TOKENS,
    },
};

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
// Minimum investment of $10,000 to become a backer if not in backer list
const MIN_DEPOSIT: Uint128 = Uint128::new(10_000_000_000u128);

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    ensure_empty_funds(&info)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = deps.api.addr_validate(&msg.admin)?;
    let config = Config {
        pool_id: 0,
        admin,
        grace_period: None,
        token_id: 0,
        token_issuer: deps.api.addr_validate(&msg.token_issuer)?,
    };
    CONFIG.save(deps.storage, &config)?;

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
        label: "Pool Token #2".to_string(),
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
        ExecuteMsg::Deposit { msg } => deposit(deps, env, info, msg),
        ExecuteMsg::Repay { msg } => repay(deps, env, info, msg),
        ExecuteMsg::Drawdown { msg } => drawdown(deps, env, info, msg),
        _ => todo!(),
    }
}

pub fn create_pool(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreatePoolMsg,
) -> ContractResult<Response> {
    // !-------
    // necessary validations
    // -------!
    // - verify all `msg` parameters
    let borrower = deps.api.addr_validate(&msg.borrower)?;
    let mut config = CONFIG.load(deps.as_ref().storage)?;
    if info.sender != borrower && info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    };
    ensure_empty_funds(&info)?;
    ensure_kyc(deps.as_ref(), borrower.clone())?;
    validate_create_pool_msg(deps.as_ref(), &info, &msg)?;

    SENIOR_POOLS
        .load(deps.storage, msg.denom.clone())
        .map_err(|_| ContractError::CustomError {
            msg: format!("Senior Pool not set for {}", msg.denom),
        })?;

    let backers = msg
        .backers
        .iter()
        .filter_map(|address| deps.api.addr_validate(address).ok())
        .collect();

    let grace_period = get_grace_period(deps.as_ref())?;

    // create credit line
    let credit_line = CreditLine::new(
        msg.borrow_limit,
        msg.term_length,
        msg.drawdown_period,
        grace_period,
        msg.principal_grace_period,
        msg.interest_apr,
        msg.late_fee_apr,
        msg.junior_fee_percent,
        msg.interest_frequency,
        msg.principal_frequency,
        &env,
    );

    // create pool
    config.pool_id += 1;
    let tranche_pool = TranchePool::new(
        config.pool_id,
        msg.pool_name,
        borrower.clone(),
        msg.borrower_name,
        msg.denom,
        backers,
        &env,
    );

    // initialize pool slice
    initialize_next_slice(deps.branch(), tranche_pool.pool_id)?;

    TRANCHE_POOLS.save(deps.storage, tranche_pool.pool_id, &tranche_pool)?;
    CREDIT_LINES.save(deps.storage, tranche_pool.pool_id, &credit_line)?;

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
    // basic contraints
    ensure_kyc(deps.as_ref(), info.sender.clone())?;
    match info.funds.len() {
        0 => return Err(ContractError::EmptyFunds),
        1 if info.funds[0].amount.is_zero() => return Err(ContractError::EmptyFunds),
        1 => {}
        _ => return Err(ContractError::MultipleTokens),
    };

    if info.funds[0].amount != msg.amount {
        return Err(ContractError::FundDiscrepancy {
            required: msg.amount,
            sent: info.funds[0].amount,
        });
    }

    let pool = TRANCHE_POOLS
        .load(deps.storage, msg.pool_id)
        .map_err(|_| ContractError::InvalidPoolId { id: msg.pool_id })?;
    if pool.denom != info.funds[0].denom {
        return Err(ContractError::CustomError {
            msg: "Incorrect denom for deposit".to_string(),
        });
    }

    // non-backer should deposit minimum set amount
    if !pool.is_backer(&info.sender) && msg.amount < MIN_DEPOSIT {
        return Err(ContractError::CustomError {
            msg: format!("Minimum deposit amount for non backers is {MIN_DEPOSIT}"),
        });
    };

    let senior_pool = SENIOR_POOLS
        .load(deps.storage, pool.denom.clone())
        .map_err(|_| ContractError::SeniorPoolNotFound {
            denom: pool.denom.clone(),
        })?;

    let mut slices = POOL_SLICES
        .load(deps.storage, msg.pool_id)
        .map_err(|_| ContractError::InvalidPoolId { id: msg.pool_id })?;
    let tranche_info = get_tranche_info(msg.tranche_id, &mut slices)?;

    // Only senior pool can deposit in senior tranche
    if tranche_info.is_senior_tranche() && info.sender != senior_pool {
        return Err(ContractError::NotSeniorPool);
    }
    tranche_info.principal_deposited = tranche_info.principal_deposited.checked_add(msg.amount)?;
    POOL_SLICES.save(deps.storage, msg.pool_id, &slices);

    let mut config = CONFIG.load(deps.storage)?;
    config.token_id += 1;
    CONFIG.save(deps.storage, &config)?;

    let mut nft = InvestorToken::new(config.token_id, msg.pool_id);
    nft.lend_info.principal_deposited += msg.amount;
    let mint_msg = CW721ExecuteMsg::<InvestorToken, Empty>::Mint(MintMsg {
        token_id: config.token_id.to_string(),
        owner: info.sender.to_string(),
        token_uri: None,
        extension: nft,
    });
    let wasm_msg = WasmMsg::Execute {
        contract_addr: config.token_issuer.to_string(),
        msg: to_json_binary(&mint_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("depositer", info.sender)
        .add_attribute("amount", msg.amount.to_string())
        .add_attribute("denom", pool.denom)
        .add_attribute("tranche_id", tranche_info.id.to_string())
        .add_message(wasm_msg))
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
    TRANCHE_POOLS.save(deps.storage, msg.pool_id, &pool)?;
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
    msg: RepayMsg,
) -> ContractResult<Response> {
    if !has_kyc(deps.as_ref(), info.sender.clone())? {
        return Err(ContractError::CustomError {
            msg: "non-KYC user".to_string(),
        });
    }
    if info.funds.is_empty() || info.funds[0].amount.is_zero() {
        return Err(ContractError::EmptyFunds);
    } else if info.funds.len() > 1 {
        return Err(ContractError::MultipleTokens);
    };
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
    let mut amount = msg.amount;
    let (pending_interest, pending_principal) = pool.repay(&mut amount, &env)?;
    TRANCHE_POOLS.save(deps.storage, msg.pool_id, &pool)?;

    // !-------
    // Handle pending_payments
    // -------!

    Ok(Response::new()
        .add_attribute("pending_interest", pending_interest)
        .add_attribute("pending_principal", pending_principal))
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
