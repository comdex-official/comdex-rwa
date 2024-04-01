// TODO
// - add txns to change the backer list
// - add txn to change the senior-pool contract addr
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_json_binary, BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Empty, Env, MessageInfo,
    Reply, Response, StdError, StdResult, SubMsg, Timestamp, Uint128, WasmMsg,
};

use cw2::set_contract_version;
use cw721_base::{
    ExecuteMsg as Cw721BaseExecuteMsg, InstantiateMsg as Cw721IntantiateMsg, MintMsg,
};
use cw_utils::parse_reply_instantiate_data;

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
    query::{get_config, get_investments, get_nft_info, get_nft_owner},
    state::{
        Config, CreditLine, InvestorToken, PoolSlice, PoolType, RepaymentInfo, TranchePool, CONFIG,
        CREDIT_LINES, KYC_CONTRACT, POOL_SLICES, REPAYMENTS, RESERVE_ADDR, SENIOR_POOLS,
        TRANCHE_POOLS, WHITELISTED_TOKENS,
    },
};

pub type Cw721ExecuteMsg = cw721_metadata_onchain::msg::ExecuteMsg<InvestorToken, Empty>;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTANTIATE_REPLY_ID: u64 = 1;

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
        token_issuer: info.sender.clone(),
        reserve_fee: msg.reserves_fee,
    };
    CONFIG.save(deps.storage, &config)?;

    let reserves_addr = deps.api.addr_validate(&msg.reserves_addr)?;
    RESERVE_ADDR.save(deps.storage, &reserves_addr)?;

    let kyc_contract = deps.api.addr_validate(&msg.kyc_addr)?;
    KYC_CONTRACT.save(deps.storage, &kyc_contract)?;

    WHITELISTED_TOKENS.save(
        deps.storage,
        msg.denom,
        &(true, 10u32.pow(msg.decimals as u32)),
    )?;

    let cw721_msg = Cw721IntantiateMsg {
        name: "PoolToken".to_string(),
        symbol: "PoTo".to_string(),
        collection_uri: None,
        minter: env.contract.address.to_string(),
        metadata: Empty {},
    };
    let instantiate_msg = CosmosMsg::Wasm(WasmMsg::Instantiate {
        admin: Some(info.sender.to_string()),
        code_id: msg.code_id,
        msg: to_json_binary(&cw721_msg)?,
        funds: vec![],
        label: "Pool Token #2".to_string(),
    });
    let submessage = SubMsg::reply_on_success(instantiate_msg, INSTANTIATE_REPLY_ID);

    Ok(Response::default().add_submessage(submessage))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        INSTANTIATE_REPLY_ID => handle_instantiate_reply(deps, msg),
        id => Err(StdError::GenericErr {
            msg: format!("Unknown reply id: {}", id),
        }),
    }
}

pub fn handle_instantiate_reply(deps: DepsMut, msg: Reply) -> StdResult<Response> {
    let res = parse_reply_instantiate_data(msg)
        .map_err(|e| StdError::generic_err(format!("Error on reply: {}", e)))?;

    let mut config = CONFIG.load(deps.as_ref().storage)?;
    config.token_issuer = deps.api.addr_validate(&res.contract_address)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("token_issuer", config.token_issuer.to_string()))
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
        ExecuteMsg::SetKycContract { addr } => set_kyc_contract(deps, env, info, addr),
        ExecuteMsg::WhitelistToken { denom, decimals } => {
            whitelist_token(deps, info, denom, decimals)
        }
        ExecuteMsg::Withdraw { msg } => withdraw(deps, env, info, msg),
        ExecuteMsg::WithdrawAll { pool_id } => withdraw_all(deps, env, info, pool_id),
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
        msg.pool_type,
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
    let mint_msg =
        Cw721ExecuteMsg::Base(Cw721BaseExecuteMsg::<InvestorToken, Empty>::Mint(MintMsg {
            token_id: config.token_id.to_string(),
            owner: info.sender.to_string(),
            token_uri: None,
            extension: nft,
        }));
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
    ensure_empty_funds(&info)?;
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
    POOL_SLICES.save(deps.storage, msg.pool_id, &slices)?;

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(msg.amount.u128(), pool.denom.clone()),
    });
    Ok(Response::new()
        .add_attribute("method", "drawdown".to_string())
        .add_attribute("borrower", info.sender.to_string())
        .add_message(msg))
}

pub fn lock_junior_capital(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: LockJuniorCapitalMsg,
) -> ContractResult<Response> {
    let mut slices = load_slices(deps.as_ref(), msg.pool_id)?;
    if slices.is_empty()
        || slices.last().unwrap().is_locked()
        || slices.last().unwrap().junior_tranche.locked_until != Timestamp::default()
    {
        return Ok(Response::new());
    }

    let cl = load_credit_line(deps.as_ref(), msg.pool_id)?;

    // calculate total junior pool deposits
    let mut junior_deposits = slices.last().unwrap().junior_tranche.principal_deposited;
    slices.last_mut().unwrap().junior_tranche.locked_until =
        env.block.time.plus_seconds(cl.drawdown_period);

    let mut res = Response::new();
    let pool = load_pool(deps.as_ref(), msg.pool_id)?;
    match pool.pool_type {
        PoolType::Junior => {}
        PoolType::Undefined => {
            //let senior_pool_addr = SENIOR_POOLS
            //.load(deps.as_ref().storage, pool.denom.clone())
            //.map_err(|_| ContractError::SeniorPoolNotFound { denom: pool.denom })?;

            // query the leverage ratio
            //let query_msg = QueryRequest::Wasm(WasmQuery::Smart {
            //contract_addr: senior_pool_addr.to_string(),
            //msg: to_json_binary(&MaxLeverageRatio {})?,
            //});
            //let leverage_ratio: Decimal = deps.querier.query(&query_msg)?;
            //// calculate senior pool amount: lr * junior deposit
            //let investment_amount =
            //leverage_ratio.checked_sub(Decimal::from_atomics(junior_deposits.u128(), 0)?)?;
            //// request the amount from senior pool
            //let execute_msg = Invest {
            //pool_id: 0,
            //tranche_id: 0,
            //amount: Uint128::zero(),
            //};
            //let cosmos_msg = WasmMsg::Execute {
            //contract_addr: senior_pool_addr.to_string(),
            //msg: to_json_binary(&execute_msg)?,
            //funds: vec![],
            //};
            //res.add_message(cosmos_msg);
        }
    };

    POOL_SLICES.save(deps.storage, msg.pool_id, &slices)?;

    Ok(res
        .add_attribute("method", "lock_junior_capital")
        .add_attribute("borrower", info.sender.to_string())
        .add_attribute("junior_capital", junior_deposits.to_string()))
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
        .add_attribute("senior_tranche", senior_tranche_id.to_string())
        .add_attribute("amount_locked", tranche_deposit.to_string()))
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
    if credit_line.borrow_info.total_borrowed.is_zero() {
        return Err(ContractError::CustomError {
            msg: "Repayment not allowed when not borrowed".to_string(),
        });
    }
    let repayment_info = credit_line.repay(msg.amount, &env)?;
    CREDIT_LINES.save(deps.storage, msg.pool_id, &credit_line)?;
    let interest_accrued = repayment_info.interest_repaid + repayment_info.interest_pending;
    REPAYMENTS.update(
        deps.storage,
        msg.pool_id,
        |val| -> ContractResult<Vec<RepaymentInfo>> {
            match val {
                None => Ok(vec![repayment_info.clone()]),
                Some(mut payments) => {
                    payments.push(repayment_info.clone());
                    Ok(payments)
                }
            }
        },
    )?;

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

    let config = get_config(deps.as_ref(), env.clone())?;
    let amount = if let Some(val) = msg.amount {
        val
    } else {
        Uint128::MAX
    };
    let mut interest_withdrawn = Uint128::zero();
    let mut principal_withdrawn = Uint128::zero();
    let mut wasm_msg: CosmosMsg<Empty>;
    if tranche_info.locked_until == Timestamp::default() {
        tranche_info.principal_deposited = tranche_info.principal_deposited.checked_sub(amount)?;
        principal_withdrawn = amount;

        let withdraw_msg = Cw721ExecuteMsg::WithdrawPrincipal {
            token_id: msg.token_id.to_string(),
            principal_amount: amount,
        };
        wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.token_issuer.to_string(),
            msg: to_json_binary(&withdraw_msg)?,
            funds: vec![],
        });
    } else {
        interest_withdrawn = std::cmp::min(interest_redeemable, amount);
        principal_withdrawn = std::cmp::min(
            principal_redeemable,
            amount.checked_sub(interest_withdrawn)?,
        );

        let redeem_msg = Cw721ExecuteMsg::Redeem {
            token_id: msg.token_id.to_string(),
            principal_redeemed: principal_withdrawn,
            interest_redeemed: interest_withdrawn,
        };
        wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.token_issuer.to_string(),
            msg: to_json_binary(&redeem_msg)?,
            funds: vec![],
        });
    }

    let pool = load_pool(deps.as_ref(), investor_token.pool_id)?;
    let bank_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(
            { interest_withdrawn + principal_withdrawn }.u128(),
            pool.denom.clone(),
        ),
    });

    POOL_SLICES.save(deps.storage, investor_token.pool_id, &slices)?;

    Ok(Response::new()
        .add_attribute("method", "withdraw")
        .add_attribute("user", info.sender.into_string())
        .add_attribute("interest_withdrawn", interest_withdrawn.to_string())
        .add_attribute("principal_withdrawn", principal_withdrawn.to_string())
        .add_attribute("denom", pool.denom)
        .add_message(bank_msg)
        .add_message(wasm_msg))
}

pub fn withdraw_all(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_id: u64,
) -> ContractResult<Response> {
    // query all nft for the user
    let investments =
        get_investments(deps.as_ref(), env.clone(), info.sender.to_string(), pool_id)?;
    let mut slices = load_slices(deps.as_ref(), pool_id)?;
    let pool = load_pool(deps.as_ref(), pool_id)?;
    let mut response = Response::new();
    let mut interest_withdrawn = Uint128::zero();
    let mut principal_withdrawn = Uint128::zero();
    for investor_token in investments.into_iter() {
        let tranche_info = get_tranche_info(investor_token.tranche_id, &mut slices)?;

        let (interest_redeemable, principal_redeemable) =
            tranche_info.redeemable_interest_and_amount(&investor_token)?;
        let total_redeemable = interest_redeemable.checked_add(principal_redeemable)?;

        //if msg.amount.is_some() && *msg.amount.as_ref().unwrap() > total_redeemable {
        //return Err(ContractError::CustomError {
        //msg: "Redemption amount exceeds redeemable".to_string(),
        //});
        //}
        if env.block.time <= tranche_info.locked_until {
            return Err(ContractError::LockedPoolWithdrawal);
        }

        let config = get_config(deps.as_ref(), env.clone())?;
        let amount = Uint128::MAX;
        let mut wasm_msg: CosmosMsg<Empty>;
        if tranche_info.locked_until == Timestamp::default() {
            tranche_info.principal_deposited =
                tranche_info.principal_deposited.checked_sub(amount)?;
            principal_withdrawn = amount;

            let withdraw_msg = Cw721ExecuteMsg::WithdrawPrincipal {
                token_id: investor_token.token_id.to_string(),
                principal_amount: amount,
            };
            wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.token_issuer.to_string(),
                msg: to_json_binary(&withdraw_msg)?,
                funds: vec![],
            });
        } else {
            interest_withdrawn = std::cmp::min(interest_redeemable, amount);
            principal_withdrawn = std::cmp::min(
                principal_redeemable,
                amount.checked_sub(interest_withdrawn)?,
            );

            let redeem_msg = Cw721ExecuteMsg::Redeem {
                token_id: investor_token.token_id.to_string(),
                principal_redeemed: principal_withdrawn,
                interest_redeemed: interest_withdrawn,
            };
            wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.token_issuer.to_string(),
                msg: to_json_binary(&redeem_msg)?,
                funds: vec![],
            });
        }

        let bank_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(
                { interest_withdrawn + principal_withdrawn }.u128(),
                pool.denom.clone(),
            ),
        });
        response = response.add_message(wasm_msg);
        response = response.add_message(bank_msg);
    }

    POOL_SLICES.save(deps.storage, pool_id, &slices)?;

    Ok(response
        .add_attribute("method", "withdraw")
        .add_attribute("user", info.sender.into_string())
        .add_attribute("interest_withdrawn", interest_withdrawn.to_string())
        .add_attribute("principal_withdrawn", principal_withdrawn.to_string())
        .add_attribute("denom", pool.denom))
}

pub fn whitelist_token(
    deps: DepsMut,
    info: MessageInfo,
    denom: String,
    decimals: u8,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    let decimals_pow = 10u32.pow(decimals as u32);
    WHITELISTED_TOKENS.save(deps.storage, denom.clone(), &(true, decimals_pow))?;

    Ok(Response::new()
        .add_attribute("method", "whitelist_token")
        .add_attribute("token", denom)
        .add_attribute("decimals", decimals.to_string()))
}

pub fn set_kyc_contract(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    addr: String,
) -> ContractResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    let new_addr = deps.api.addr_validate(&addr)?;
    KYC_CONTRACT.save(deps.storage, &new_addr)?;

    Ok(Response::new()
        .add_attribute("method", "set_kyc_contract".to_string())
        .add_attribute("new_addr", addr))
}

pub fn load_slices(deps: Deps, pool_id: u64) -> ContractResult<Vec<PoolSlice>> {
    Ok(POOL_SLICES
        .load(deps.storage, pool_id)
        .map_err(|_| ContractError::CustomError {
            msg: format!("Unable to load tranches for given pool_id: {}", pool_id),
        })?)
}

pub fn load_credit_line(deps: Deps, pool_id: u64) -> ContractResult<CreditLine> {
    Ok(CREDIT_LINES
        .load(deps.storage, pool_id)
        .map_err(|_| ContractError::CustomError {
            msg: format!("Unable to load credit line for given pool_id: {}", pool_id),
        })?)
}

pub fn load_pool(deps: Deps, pool_id: u64) -> ContractResult<TranchePool> {
    Ok(TRANCHE_POOLS
        .load(deps.storage, pool_id)
        .map_err(|_| ContractError::CustomError {
            msg: format!("Unable to load pool with given pool_id: {}", pool_id),
        })?)
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

#[cfg(test)]
mod tests {
    use crate::{
        state::{BorrowInfo, PaymentFrequency},
        GRACE_PERIOD,
    };

    use super::*;
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        Addr,
    };

    const USDC: &str = "usdc";

    fn init_msg() -> InstantiateMsg {
        InstantiateMsg {
            admin: "admin".to_string(),
            code_id: 15,
            reserves_fee: 1000,
            reserves_addr: "reserves".to_string(),
            kyc_addr: "kyc_contract".to_string(),
            denom: "usdc".to_string(),
            decimals: 6,
        }
    }

    fn create_pool_msg() -> CreatePoolMsg {
        CreatePoolMsg {
            pool_name: "Demo Pool 1".to_string(),
            borrower: "borrower".to_string(),
            borrower_name: "Borrower 1".to_string(),
            uid_token: Uint128::zero(),
            interest_apr: 500,
            junior_fee_percent: 2000,
            late_fee_apr: 1000,
            borrow_limit: Uint128::new(1000000000000),
            interest_frequency: PaymentFrequency::Monthly,
            principal_frequency: PaymentFrequency::Monthly,
            principal_grace_period: 7776000,
            drawdown_period: 1209600,
            term_length: 31536000,
            denom: USDC.to_string(),
            backers: vec![],
            pool_type: PoolType::Junior,
        }
    }

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("admin", &[]);

        let msg = init_msg();
        let result = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap();

        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        assert_eq!(config.pool_id, 0u64);
        assert_eq!(config.token_id, 0u128);
        assert_eq!(config.admin, Addr::unchecked("admin"));
        assert_eq!(config.grace_period, None);
        assert_eq!(config.reserve_fee, 1000u16);
        // token issuer is admin because the reply is not called in testing
        assert_eq!(config.token_issuer, Addr::unchecked("admin"));
    }

    #[test]
    fn test_create_pool() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("admin", &[]);

        let msg = init_msg();
        let result = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let borrower = Addr::unchecked("borrower");

        SENIOR_POOLS
            .save(
                deps.as_mut().storage,
                USDC.to_string(),
                &Addr::unchecked("usdc_senior_pool"),
            )
            .unwrap();

        let pool_msg = create_pool_msg();
        let info = mock_info(borrower.as_str(), &[]);
        let response =
            create_pool(deps.as_mut(), env.clone(), info.clone(), pool_msg.clone()).unwrap();

        const POOL_ID: u64 = 1u64;

        let pool = TRANCHE_POOLS.load(deps.as_ref().storage, POOL_ID).unwrap();
        assert_eq!(pool.pool_id, POOL_ID);
        assert_eq!(pool.pool_name, pool_msg.pool_name);
        assert_eq!(pool.pool_type, pool_msg.pool_type);
        assert_eq!(pool.borrower_addr, borrower);
        assert_eq!(pool.borrower_name, pool_msg.borrower_name);

        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        assert_eq!(config.pool_id, POOL_ID);

        let cl = CREDIT_LINES.load(deps.as_ref().storage, POOL_ID).unwrap();
        assert_eq!(cl.term_start, Timestamp::default());
        assert_eq!(cl.term_end, Timestamp::default());
        assert_eq!(cl.term_length, pool_msg.term_length);
        assert_eq!(cl.principal_grace_period, pool_msg.principal_grace_period);
        assert_eq!(cl.drawdown_period, pool_msg.drawdown_period);
        let mut borrow_info = BorrowInfo::default();
        borrow_info.borrow_limit = pool_msg.borrow_limit;
        assert_eq!(cl.borrow_info, borrow_info);
        assert_eq!(cl.interest_apr, pool_msg.interest_apr);
        assert_eq!(cl.junior_fee_percent, pool_msg.junior_fee_percent);
        assert_eq!(cl.late_fee_apr, pool_msg.late_fee_apr);
        assert_eq!(cl.interest_frequency, pool_msg.interest_frequency);
        assert_eq!(cl.principal_frequency, pool_msg.principal_frequency);
        assert_eq!(cl.last_update_ts, env.block.time);
        assert_eq!(cl.last_full_payment, Timestamp::default());
        assert_eq!(cl.interest_owed, Uint128::zero());
        assert_eq!(cl.interest_accrued, Uint128::zero());
        if config.grace_period.is_some() {
            assert_eq!(cl.grace_period, config.grace_period.unwrap());
        } else {
            assert_eq!(cl.grace_period, GRACE_PERIOD);
        };

        // !-------
        // check slices
        // -------!
    }

    #[test]
    fn test_deposit() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("admin", &[]);

        let msg = init_msg();
        let result = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let borrower = Addr::unchecked("borrower");

        SENIOR_POOLS
            .save(
                deps.as_mut().storage,
                USDC.to_string(),
                &Addr::unchecked("usdc_senior_pool"),
            )
            .unwrap();

        let pool_msg = create_pool_msg();
        let info = mock_info(borrower.as_str(), &[]);
        let response = create_pool(deps.as_mut(), env.clone(), info.clone(), pool_msg).unwrap();

        assert!(TRANCHE_POOLS.has(deps.as_ref().storage, 1u64));
        assert!(POOL_SLICES.has(deps.as_ref().storage, 1));

        let deposit_msg = DepositMsg {
            amount: Uint128::new(10000000000),
            pool_id: 1,
            tranche_id: 0,
        };
        let info = mock_info("backer1", &coins(10000000000, USDC));
        let response = deposit(deps.as_mut(), env.clone(), info, deposit_msg).unwrap();

        // Slices
        // Credit Line
    }

    #[test]
    fn test_drawdown_soon_after_pool_creation() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("admin", &[]);

        let msg = init_msg();
        let result = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let borrower = Addr::unchecked("borrower");

        SENIOR_POOLS
            .save(
                deps.as_mut().storage,
                USDC.to_string(),
                &Addr::unchecked("usdc_senior_pool"),
            )
            .unwrap();

        let pool_msg = create_pool_msg();
        let info = mock_info(borrower.as_str(), &[]);
        let response = create_pool(deps.as_mut(), env.clone(), info.clone(), pool_msg).unwrap();

        assert!(TRANCHE_POOLS.has(deps.as_ref().storage, 1u64));

        let drawdown_msg = DrawdownMsg {
            amount: Uint128::new(1000),
            pool_id: 1,
        };
        let info = mock_info(borrower.as_str(), &[]);
        let response = drawdown(deps.as_mut(), env.clone(), info, drawdown_msg).unwrap_err();
        match response {
            ContractError::CustomError { msg } if msg == "Pool not locked".to_string() => {}
            e => panic!("Unexepected error: {}", e),
        }
    }

    #[test]
    fn test_drawdown() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("admin", &[]);

        let msg = init_msg();
        let result = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let borrower = Addr::unchecked("borrower");

        WHITELISTED_TOKENS
            .save(deps.as_mut().storage, USDC.to_string(), &(true, 1000000))
            .unwrap();

        SENIOR_POOLS
            .save(
                deps.as_mut().storage,
                USDC.to_string(),
                &Addr::unchecked("usdc_senior_pool"),
            )
            .unwrap();

        // Create Pool
        let pool_msg = create_pool_msg();
        let info = mock_info(borrower.as_str(), &[]);
        let response = create_pool(deps.as_mut(), env.clone(), info.clone(), pool_msg).unwrap();

        // Deposit
        let info = mock_info("user1", &coins(MIN_DEPOSIT.u128(), USDC.to_string()));
        let deposit_msg = DepositMsg {
            pool_id: 1,
            tranche_id: 0,
            amount: info.funds[0].amount.clone(),
        };
        let response = deposit(deps.as_mut(), env.clone(), info, deposit_msg).unwrap();

        // Lock junior capital
        let info = mock_info(borrower.as_str(), &[]);
        let lock_junior_capital_msg = LockJuniorCapitalMsg { pool_id: 1 };
        let response =
            lock_junior_capital(deps.as_mut(), env.clone(), info, lock_junior_capital_msg).unwrap();

        // Lock Pool
        let info = mock_info(borrower.as_str(), &[]);
        let lock_pool_msg = LockPoolMsg { pool_id: 1 };
        let response = lock_pool(deps.as_mut(), env.clone(), info, lock_pool_msg).unwrap();

        let slices = load_slices(deps.as_ref(), 1).unwrap();
        assert_eq!(slices[0].junior_tranche.principal_deposited, MIN_DEPOSIT);
        assert_eq!(
            slices[0].junior_tranche.principal_deposited
                * slices[0].junior_tranche.principal_share_price,
            slices[0].junior_tranche.principal_deposited,
            "Junior Tranche calc error"
        );

        assert_eq!(
            slices[0].senior_tranche.principal_deposited
                * slices[0].senior_tranche.principal_share_price,
            slices[0].senior_tranche.principal_deposited,
            "Senior Tranche calc error"
        );

        // Drawdown
        let drawdown_msg = DrawdownMsg {
            amount: Uint128::new(10000),
            pool_id: 1,
        };
        let info = mock_info(borrower.as_str(), &[]);
        let response = drawdown(deps.as_mut(), env.clone(), info, drawdown_msg).unwrap();
    }
}
