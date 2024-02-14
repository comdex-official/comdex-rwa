#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, CosmosMsg, DepsMut, Env, MessageInfo, Response, WasmMsg};

use cw2::set_contract_version;
use cw20::MinterResponse;
use cw20_base::{
    self,
    msg::{ExecuteMsg as Cw20ExecuteMsg, InstantiateMsg as Cw20InstantiateMsg},
};

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
