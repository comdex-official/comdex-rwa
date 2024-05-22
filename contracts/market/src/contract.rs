#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    QueryRequest, Response, StdResult, Uint128, WasmMsg, WasmQuery,
};

use cw20::{BalanceResponse, Cw20ExecuteMsg};
use cw20_base::msg::QueryMsg as Cw20QueryMsg;
use cw721::{Cw721QueryMsg, OwnerOfResponse};
use cw721_base::ExecuteMsg as Cw721BaseExecuteMsg;
use cw721_metadata_onchain::ExecuteMsg as Cw721ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Asset, Config, Order, Status, ADMIN, CONFIG, ORDERS, SENIOR_POOLS};

// version info for migration info
const CONTRACT_NAME: &str = "market";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        nft_contract: deps.api.addr_validate(&msg.nft_contract)?,
        order_id: 1000000,
    };

    CONFIG.save(deps.storage, &config)?;

    for (denom, contract_addr) in msg.senior_pools.iter() {
        SENIOR_POOLS.save(
            deps.storage,
            denom.to_owned(),
            &deps.api.addr_validate(contract_addr)?,
        )?;
    }

    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.set(deps, Some(admin))?;

    Ok(Response::new()
        .add_attributes(vec![
            ("method", "instantiate"),
            ("admin", &msg.admin),
            ("nft_contract", &msg.nft_contract),
        ])
        .add_attributes(msg.senior_pools))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::NftSellOrder { token_id, price } => {
            nft_sell_order(deps, env, info, token_id, price)
        }
        ExecuteMsg::TokenSellOrder {
            amount,
            denom,
            price,
        } => token_sell_order(deps, env, info, amount, denom, price),
        ExecuteMsg::BuyOrder { order_id } => execute_buy_order(deps, env, info, order_id),
        ExecuteMsg::CancelOrder { order_id } => cancel_order(deps, env, info, order_id),
    }
}

/// Working:
/// 1. Check if enough cw20 is owned by the user
/// 2. Create a new sell order
/// 3. Transfer cw20 to this contract, requires that the contract is allowed by the user to
///    transfer tokens.
pub fn token_sell_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    denom: String,
    price: Coin,
) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::CustomError {
            msg: "Funds not allowed".to_string(),
        });
    }

    let mut config = CONFIG.load(deps.storage)?;
    config.order_id += 1;
    CONFIG.save(deps.storage, &config)?;

    // verify that the user has the asset to sell
    // A. if asset is nft, query the nft and check owner
    // B. if asset is cw20, query the balance of sender.
    let token_contract = SENIOR_POOLS.load(deps.storage, denom.clone())?;
    let query = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_contract.to_string(),
        msg: to_json_binary(&Cw20QueryMsg::Balance {
            address: info.sender.to_string(),
        })?,
    });
    let result = deps.querier.query::<BalanceResponse>(&query)?;

    if result.balance.u128() != amount.u128() {
        return Err(ContractError::CustomError {
            msg: "Not enough token balance".to_string(),
        });
    }

    let order = Order {
        id: config.order_id,
        price: price.clone(),
        asset_class: Asset::Cw20 {
            denom: denom.clone(),
            amount,
        },
        seller: info.sender.clone(),
        status: Status::Pending,
    };
    ORDERS.save(deps.storage, order.id, &order)?;

    // Transfer tokens from user to contract.
    let cw20_transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.to_string(),
        recipient: env.contract.address.to_string(),
        amount,
    };
    let wasm_execute_msg = WasmMsg::Execute {
        contract_addr: token_contract.to_string(),
        msg: to_json_binary(&cw20_transfer_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_attributes(vec![
            ("method", "token_sell_order"),
            ("order_id", &order.id.to_string()),
            ("seller", info.sender.as_str()),
            ("asset_denom", &denom),
            ("asset_amount", &amount.to_string()),
            ("price_denom", &price.denom),
            ("price", &price.amount.to_string()),
        ])
        .add_message(wasm_execute_msg))
}

/// Working:
/// 1. Check if the nft is owned by the user
/// 2. Create a new sell order
/// 3. Transfer the nft to this contract, requires that the contract is allowed by the user to
///    transfer the nft.
pub fn nft_sell_order(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: String,
    price: Coin,
) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::CustomError {
            msg: "Funds not allowed".to_string(),
        });
    }

    let mut config = CONFIG.load(deps.storage)?;
    config.order_id += 1;
    CONFIG.save(deps.storage, &config)?;

    // verify that the user has the asset to sell
    // A. if asset is nft, query the nft and check owner
    // B. if asset is cw20, query the balance of sender.
    let query = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.nft_contract.to_string(),
        msg: to_json_binary(&Cw721QueryMsg::OwnerOf {
            token_id: token_id.clone(),
            include_expired: None,
        })?,
    });
    let result = deps.querier.query::<OwnerOfResponse>(&query)?;

    if result.owner != info.sender.to_string() {
        return Err(ContractError::CustomError {
            msg: "Attempt to list NFT of a different user".to_string(),
        });
    }

    let order = Order {
        id: config.order_id,
        price: price.clone(),
        asset_class: Asset::Nft(token_id.clone()),
        seller: info.sender.clone(),
        status: Status::Pending,
    };
    ORDERS.save(deps.storage, order.id, &order)?;

    let transfer_msg = WasmMsg::Execute {
        contract_addr: config.nft_contract.to_string(),
        msg: to_json_binary(&Cw721ExecuteMsg::Base(Cw721BaseExecuteMsg::TransferNft {
            recipient: env.contract.address.to_string(),
            token_id: order.asset_class.get_nft_id()?,
        }))?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_attributes(vec![
            ("method", "token_sell_order"),
            ("order_id", &order.id.to_string()),
            ("seller", info.sender.as_str()),
            ("price_denom", &price.denom),
            ("price", &price.amount.to_string()),
        ])
        .add_message(transfer_msg))
}

/// Working:
/// 1. Check if the order exists and is pending
/// 2. Funds should match the sell price
/// 3. Transfer asset to the buyer
/// 4. Transfer funds to the seller
pub fn execute_buy_order(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    order_id: u64,
) -> Result<Response, ContractError> {
    match info.funds.len() {
        0 => {
            return Err(ContractError::CustomError {
                msg: "Empty funds".to_string(),
            })
        }
        1 if info.funds[0].amount.is_zero() => {
            return Err(ContractError::CustomError {
                msg: "Empty funds".to_string(),
            })
        }
        1 => (),
        _ => {
            return Err(ContractError::CustomError {
                msg: "Multiple denom not allowed".to_string(),
            })
        }
    };

    let mut order = ORDERS.load(deps.storage, order_id)?;

    if order.price.amount.u128() != info.funds[0].amount.u128() {
        return Err(ContractError::CustomError {
            msg: "Sent amount does not match the ask price".to_string(),
        });
    } else if order.price.denom != info.funds[0].denom {
        return Err(ContractError::CustomError {
            msg: "Sent denom does not match the ask price denom".to_string(),
        });
    }

    match order.status {
        Status::Cancelled | Status::Completed => {
            return Err(ContractError::CustomError {
                msg: "Order has been cancelled or fulfilled".to_string(),
            })
        }
        Status::Pending => {}
    };
    order.status = Status::Completed;
    ORDERS.save(deps.storage, order_id, &order)?;

    let transfer_msg: CosmosMsg = if order.asset_class.is_cw20() {
        let token_contract =
            SENIOR_POOLS.load(deps.storage, order.asset_class.get_token_denom()?)?;

        WasmMsg::Execute {
            contract_addr: token_contract.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: order.asset_class.get_token_amount()?,
            })?,
            funds: vec![],
        }
        .into()
    } else {
        let config = CONFIG.load(deps.storage)?;

        WasmMsg::Execute {
            contract_addr: config.nft_contract.to_string(),
            msg: to_json_binary(&Cw721ExecuteMsg::Base(Cw721BaseExecuteMsg::TransferNft {
                recipient: info.sender.to_string(),
                token_id: order.asset_class.get_nft_id()?,
            }))?,
            funds: vec![],
        }
        .into()
    };
    // Transfer funds to seller
    let bank_msg = BankMsg::Send {
        to_address: order.seller.to_string(),
        amount: info.funds.clone(),
    }
    .into();

    Ok(Response::new()
        .add_attributes(vec![
            ("method", "execute_buy_order"),
            ("order_id", &order.id.to_string()),
            ("buyer", info.sender.as_str()),
            ("seller", order.seller.as_str()),
        ])
        .add_messages(vec![bank_msg, transfer_msg]))
}

pub fn cancel_order(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    order_id: u64,
) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::CustomError {
            msg: "Funds not allowed".to_string(),
        });
    }

    let mut order = ORDERS
        .may_load(deps.storage, order_id)?
        .ok_or(ContractError::CustomError {
            msg: "Invalid order ID".to_string(),
        })?;

    if order.seller != info.sender {
        return Err(ContractError::CustomError {
            msg: "Only seller can cancel the order".to_string(),
        });
    }

    order.status = Status::Cancelled;

    ORDERS.save(deps.storage, order_id, &order)?;

    // transfer the asset back to seller
    let transfer_msg: CosmosMsg = match order.asset_class {
        Asset::Nft(token_id) => {
            let config = CONFIG.load(deps.storage)?;

            WasmMsg::Execute {
                contract_addr: config.nft_contract.to_string(),
                msg: to_json_binary(&Cw721ExecuteMsg::Base(Cw721BaseExecuteMsg::TransferNft {
                    recipient: order.seller.to_string(),
                    token_id,
                }))?,
                funds: vec![],
            }
            .into()
        }
        Asset::Cw20 { denom, amount } => {
            let token_contract = SENIOR_POOLS.load(deps.storage, denom.clone())?;

            WasmMsg::Execute {
                contract_addr: token_contract.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: order.seller.to_string(),
                    amount,
                })?,
                funds: vec![],
            }
            .into()
        }
    };

    Ok(Response::new()
        .add_attributes(vec![
            ("method", "cancel_order"),
            ("seller", order.seller.as_str()),
        ])
        .add_message(transfer_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg(test)]
mod tests {}
