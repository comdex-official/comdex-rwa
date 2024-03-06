// TODO
// - add txns to change the backer list
// - add txn to change the senior-pool contract addr
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_json_binary, BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Empty, Env, MessageInfo,
    QueryRequest, Response, StdError, Timestamp, Uint128, WasmMsg, WasmQuery,
};

use cw2::set_contract_version;
use cw721_base::{ExecuteMsg as CW721ExecuteMsg, InstantiateMsg as Cw721IntantiateMsg, MintMsg};

use crate::{
    error::{ContractError, ContractResult},
    helpers::{
        apply_to_all_slices, ensure_drawdown_unpaused, ensure_empty_funds, ensure_kyc,
        get_grace_period, get_tranche_info, initialize_next_slice, scale_by_fraction,
        share_price_to_usdc, validate_create_pool_msg,
    },
    msg::{
        CreatePoolMsg, DepositMsg, DrawdownMsg, ExecuteMsg, InstantiateMsg, LockJuniorCapitalMsg,
        LockPoolMsg, MigrateMsg, RepayMsg, WithdrawMsg,
    },
    query::{get_config, get_nft_info, get_nft_owner},
    state::{
        Config, CreditLine, InvestorToken, PoolSlice, TranchePool, CONFIG, CREDIT_LINES,
        POOL_SLICES, RESERVE_ADDR, SENIOR_POOLS, TRANCHE_POOLS, WHITELISTED_TOKENS,
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
        reserve_fee: msg.reserves_fee,
    };
    CONFIG.save(deps.storage, &config)?;

    let reserves_addr = deps.api.addr_validate(&msg.reserves_addr)?;
    RESERVE_ADDR.save(deps.storage, &reserves_addr)?;

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
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::NewPool { msg } => create_pool(deps, env, info, msg),
        ExecuteMsg::Deposit { msg } => deposit(deps, env, info, msg),
        ExecuteMsg::Repay { msg } => repay(deps, env, info, msg),
        ExecuteMsg::Drawdown { msg } => drawdown(deps, env, info, msg),
        ExecuteMsg::LockPool { msg } => lock_pool(deps, env, info, msg),
        ExecuteMsg::LockJuniorCapital { msg } => lock_junior_capital(deps, env, info, msg),
        _ => todo!(),
    }
}

pub fn create_pool(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreatePoolMsg,
) -> ContractResult<Response> {
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
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "create_pool")
        .add_attribute("pool_id", config.pool_id.to_string()))
}

pub fn deposit(
    deps: DepsMut,
    _env: Env,
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

    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
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

    let mut slices = load_slices(deps.as_ref(), msg.pool_id)?;
    let tranche_info = get_tranche_info(msg.tranche_id, &mut slices)?;
    let tranche_id = tranche_info.id;

    // Only senior pool can deposit in senior tranche
    if tranche_info.is_senior_tranche() && info.sender != senior_pool {
        return Err(ContractError::NotSeniorPool);
    }
    tranche_info.principal_deposited = tranche_info.principal_deposited.checked_add(msg.amount)?;
    POOL_SLICES.save(deps.storage, msg.pool_id, &slices)?;

    let mut config = CONFIG.load(deps.storage)?;
    config.token_id += 1;
    CONFIG.save(deps.storage, &config)?;

    let mut nft = InvestorToken::new(config.token_id, msg.pool_id, tranche_id);
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
        .add_attribute("tranche_id", tranche_id.to_string())
        .add_message(wasm_msg))
}

pub fn drawdown(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: DrawdownMsg,
) -> ContractResult<Response> {
    ensure_drawdown_unpaused(deps.as_ref())?;
    ensure_kyc(deps.as_ref(), info.sender.clone())?;

    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
    if info.sender != pool.borrower_addr {
        return Err(ContractError::Unauthorized {});
    }

    let mut slices = load_slices(deps.as_ref(), msg.pool_id)?;
    if slices.is_empty() {
        return Err(ContractError::CustomError {
            msg: "Tranches not initialized".to_string(),
        });
    }
    if !slices[slices.len() - 1].is_locked() {
        return Err(ContractError::CustomError {
            msg: "Pool not locked".to_string(),
        });
    }

    let slices_len = slices.len();
    let top_slice = &mut slices[slices_len - 1];
    let mut tranche_funds = share_price_to_usdc(
        top_slice.junior_tranche.principal_share_price,
        top_slice.junior_tranche.principal_deposited,
    )?;
    tranche_funds = tranche_funds.checked_add(share_price_to_usdc(
        top_slice.senior_tranche.principal_share_price,
        top_slice.senior_tranche.principal_deposited,
    )?)?;

    if msg.amount > tranche_funds {
        return Err(ContractError::DrawdownExceedsLimit {
            limit: tranche_funds,
        });
    }

    // drawdown in creditline
    let mut credit_line = load_credit_line(deps.as_ref(), msg.pool_id)?;
    credit_line.drawdown(msg.amount, &env)?;
    CREDIT_LINES.save(deps.storage, msg.pool_id, &credit_line)?;

    // update share price in both tranches
    let remaining_amount = tranche_funds.checked_sub(msg.amount)?;
    top_slice.junior_tranche.principal_share_price = top_slice
        .junior_tranche
        .expected_share_price(remaining_amount, &top_slice)?;
    top_slice.senior_tranche.principal_share_price = top_slice
        .senior_tranche
        .expected_share_price(remaining_amount, &top_slice)?;
    top_slice.principal_deployed = top_slice.principal_deployed.checked_add(msg.amount)?;

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(msg.amount.u128(), pool.denom.clone()),
    });
    Ok(Response::new().add_message(msg))
}

pub fn lock_junior_capital(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: LockJuniorCapitalMsg,
) -> ContractResult<Response> {
    let mut slices = load_slices(deps.as_ref(), msg.pool_id)?;
    if slices.is_empty() {
        return Ok(Response::new());
    }

    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
    let senior_pool_addr = SENIOR_POOLS
        .load(deps.as_ref().storage, pool.denom.clone())
        .map_err(|_| ContractError::SeniorPoolNotFound { denom: pool.denom })?;

    // calculate total junior pool deposits
    let mut junior_deposits = Uint128::zero();
    for slice in slices.iter() {
        junior_deposits = junior_deposits.checked_add(slice.junior_tranche.principal_deposited)?;
    }
    // query the leverage ratio
    let query_msg = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: senior_pool_addr.to_string(),
        msg: to_json_binary(&MaxLeverageRatio {})?,
    });
    let leverage_ratio: Decimal = deps.querier.query(&query_msg)?;
    // calculate senior pool amount: lr * junior deposit
    let investment_amount =
        leverage_ratio.checked_sub(Decimal::from_atomics(junior_deposits.u128(), 0)?)?;
    // request the amount from senior pool
    let execute_msg = Invest {
        pool_id: 0,
        tranche_id: 0,
        amount: Uint128::zero(),
    };
    let cosmos_msg = WasmMsg::Execute {
        contract_addr: senior_pool_addr.to_string(),
        msg: to_json_binary(&execute_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_attribute("method", "lock_junior_capital")
        .add_attribute("borrower", info.sender.to_string())
        .add_attribute("junior_capital", junior_deposits.to_string())
        .add_message(cosmos_msg))
}

pub fn lock_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: LockPoolMsg,
) -> ContractResult<Response> {
    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
    if pool.borrower_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    let mut slices = load_slices(deps.as_ref(), msg.pool_id)?;
    if slices.is_empty() {
        return Ok(Response::default());
    }
    let slices_len = slices.len();
    let top_slice = &mut slices[slices_len - 1];
    if top_slice.junior_tranche.locked_until == Timestamp::default() {
        return Err(ContractError::CustomError {
            msg: "Junior tranche not locked".to_string(),
        });
    }
    if top_slice.senior_tranche.locked_until != Timestamp::default() {
        return Err(ContractError::CustomError {
            msg: "Senior tranche already locked".to_string(),
        });
    }
    let junior_tranche_id = top_slice.junior_tranche.id;
    let senior_tranche_id = top_slice.senior_tranche.id;

    let tranche_deposit = top_slice
        .junior_tranche
        .principal_deposited
        .checked_add(top_slice.senior_tranche.principal_deposited)?;

    let mut credit_line = load_credit_line(deps.as_ref(), msg.pool_id)?;
    credit_line.set_limit(std::cmp::min(
        credit_line.limit(&env)?.checked_add(tranche_deposit)?,
        credit_line.max_limit(),
    ))?;
    CREDIT_LINES.save(deps.storage, msg.pool_id, &credit_line)?;

    // lock tranches
    top_slice
        .junior_tranche
        .lock_tranche(&env, credit_line.drawdown_period)?;
    top_slice
        .senior_tranche
        .lock_tranche(&env, credit_line.drawdown_period)?;
    POOL_SLICES.save(deps.storage, msg.pool_id, &slices)?;

    Ok(Response::new()
        .add_attribute("method", "lock_pool")
        .add_attribute("borrower", info.sender.to_string())
        .add_attribute("pool_id", msg.pool_id.to_string())
        .add_attribute("junior_tranche", junior_tranche_id.to_string())
        .add_attribute("senior_tranche", senior_tranche_id.to_string()))
}

pub fn repay(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: RepayMsg,
) -> ContractResult<Response> {
    ensure_kyc(deps.as_ref(), info.sender.clone())?;
    if info.funds.is_empty() || info.funds[0].amount.is_zero() || msg.amount.is_zero() {
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
    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
    if info.funds[0].denom != pool.denom {
        return Err(ContractError::CustomError {
            msg: "Incorrect denom used for repayment".to_string(),
        });
    }
    if info.sender != pool.borrower_addr {
        return Err(ContractError::Unauthorized {});
    }

    let mut credit_line = load_credit_line(deps.as_ref(), msg.pool_id)?;
    let repayment_info = credit_line.repay(msg.amount, &env)?;
    CREDIT_LINES.save(deps.storage, msg.pool_id, &credit_line)?;
    let interest_accrued = repayment_info.interest_repaid + repayment_info.interest_pending;

    let mut res = Response::new();

    let cosmos_msg;
    if !repayment_info.excess_amount.is_zero() {
        cosmos_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(
                repayment_info.excess_amount.u128(),
                info.funds[0].denom.clone(),
            ),
        });
        res = res.add_message(cosmos_msg);
    }

    let mut slices = load_slices(deps.as_ref(), msg.pool_id)?;
    let mut principal_payment_per_slice = Vec::<Uint128>::with_capacity(slices.len());
    for slice in slices.iter_mut() {
        let interest_for_slice = scale_by_fraction(
            Decimal::from_atomics(interest_accrued, 0)?,
            slice.principal_deployed,
            credit_line.borrow_info.borrowed_amount,
        )?;
        slice.total_interest_accrued += interest_for_slice.to_uint_floor();
        principal_payment_per_slice.push(
            scale_by_fraction(
                Decimal::from_atomics(repayment_info.principal_repaid, 0)?,
                slice.principal_deployed,
                credit_line.borrow_info.borrowed_amount,
            )?
            .to_uint_floor(),
        );
    }

    let config = get_config(deps.as_ref(), env.clone())?;

    let reserve_transfer_msg;
    if !repayment_info.interest_repaid.is_zero() || !repayment_info.principal_repaid.is_zero() {
        // collect interest and principal and send to reserve
        let reserve_amount = collect_interest_and_principal(
            &mut slices,
            repayment_info.interest_repaid,
            repayment_info.principal_repaid,
            config.reserve_fee,
            credit_line.borrow_info.borrowed_amount,
            credit_line.junior_fee_percent,
            &credit_line,
            &env,
        )?;
        let reserves_addr = RESERVE_ADDR.load(deps.as_ref().storage)?;
        reserve_transfer_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: reserves_addr.into_string(),
            amount: coins(reserve_amount.u128(), pool.denom.clone()),
        });
        res = res.add_message(reserve_transfer_msg);
    }

    Ok(res
        .add_attribute("method", "repay")
        .add_attribute("borrower", info.sender.to_string())
        .add_attribute(
            "interest_repaid",
            repayment_info.interest_repaid.to_string(),
        )
        .add_attribute(
            "principal_repaid",
            repayment_info.principal_repaid.to_string(),
        )
        .add_attribute(
            "interest_pending",
            repayment_info.interest_pending.to_string(),
        )
        .add_attribute(
            "principal_pending",
            repayment_info.principal_pending.to_string(),
        ))
}

pub fn collect_interest_and_principal(
    slices: &mut Vec<PoolSlice>,
    interest: Uint128,
    principal: Uint128,
    reserve_fee: u16,
    total_deployed: Uint128,
    junior_fee_percent: u16,
    credit_line: &CreditLine,
    env: &Env,
) -> ContractResult<Uint128> {
    apply_to_all_slices(
        slices,
        interest,
        principal,
        reserve_fee,
        total_deployed,
        junior_fee_percent,
        credit_line,
        env,
    )
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: WithdrawMsg,
) -> ContractResult<Response> {
    let nft_owner = get_nft_owner(deps.as_ref(), env.clone(), msg.token_id)?;
    if nft_owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    let investor_token = get_nft_info(deps.as_ref(), env.clone(), msg.token_id)?;

    let mut slices = load_slices(deps.as_ref(), investor_token.pool_id)?;
    let tranche_info = get_tranche_info(investor_token.tranche_id, &mut slices)?;
    let (interest_redeemable, principal_redeemable) =
        tranche_info.redeemable_interest_and_amount(&investor_token)?;
    let total_redeemable = interest_redeemable.checked_add(principal_redeemable)?;

    if msg.amount.is_some() && *msg.amount.as_ref().unwrap() > total_redeemable {
        return Err(ContractError::CustomError {
            msg: "Redemption amount exceeds redeemable".to_string(),
        });
    }
    if env.block.time <= tranche_info.locked_until {
        return Err(ContractError::LockedPoolWithdrawal);
    }
    // !-------
    // REMOVE THIS; allow withdrawals before locking
    // -------!
    let mut interest_withdrawn = Uint128::zero();
    let mut principal_withdrawn = Uint128::zero();
    let amount = if let Some(val) = msg.amount {
        val
    } else {
        Uint128::MAX
    };
    if tranche_info.locked_until == Timestamp::default() {
        return Err(ContractError::CustomError {
            msg: "Withdrawal before locking not supported".to_string(),
        });
    } else {
        interest_withdrawn = std::cmp::min(interest_redeemable, amount);
        principal_withdrawn =
            std::cmp::min(principal_redeemable, amount.checked_sub(interest_withdrawn)?);
    }

    let pool = load_pool(deps.as_ref(), investor_token.pool_id)?;
    let bank_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(
            { interest_withdrawn + principal_withdrawn }.u128(),
            pool.denom.clone(),
        ),
    });

    Ok(Response::new()
        .add_attribute("method", "withdraw")
        .add_attribute("user", info.sender.into_string())
        .add_attribute("interest_withdrawn", interest_withdrawn.to_string())
        .add_attribute("principal_withdrawn", principal_withdrawn.to_string())
        .add_attribute("denom", pool.denom)
        .add_message(bank_msg))
}

pub fn load_slices(deps: Deps, pool_id: u64) -> ContractResult<Vec<PoolSlice>> {
    Ok(POOL_SLICES
        .load(deps.storage, pool_id)
        .map_err(|_| ContractError::InvalidPoolId { id: pool_id })?)
}

pub fn load_credit_line(deps: Deps, pool_id: u64) -> ContractResult<CreditLine> {
    Ok(CREDIT_LINES
        .load(deps.storage, pool_id)
        .map_err(|_| ContractError::InvalidPoolId { id: pool_id })?)
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
