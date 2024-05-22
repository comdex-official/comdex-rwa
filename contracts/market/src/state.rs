use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Uint128};
use cw_controllers::Admin;
use cw_storage_plus::{Item, Map};

use crate::ContractError;

#[cw_serde]
pub struct Config {
    pub nft_contract: Addr,
    pub order_id: u64,
}

#[cw_serde]
pub enum Asset {
    Nft(String),
    Cw20 {
        denom: String,
        amount: Uint128,
    }
}

impl Asset {
    pub fn get_nft_id(&self) -> Result<String, ContractError> {
        match self {
            Asset::Nft(token_id) => Ok(token_id.clone()),
            Asset::Cw20 {..} => Err(ContractError::CustomError {
                msg: "No token id for Cw20".to_string(),
            }),
        }
    }

    pub fn get_token_denom(&self) -> Result<String, ContractError> {
        match self {
            Asset::Cw20 { denom, .. } => Ok(denom.to_owned()),
            Asset::Nft(_) => Err(ContractError::CustomError {
                msg: "No denom for nft token".to_string(),
            }),
        }
    }

    pub fn get_token_amount(&self) -> Result<Uint128, ContractError> {
        match self {
            Asset::Cw20 { amount, .. } => Ok(amount.to_owned()),
            Asset::Nft(_) => Err(ContractError::CustomError {
                msg: "No denom for nft token".to_string(),
            }),
        }
    }

    pub fn is_nft(&self) -> bool {
        match self {
            Asset::Nft(_) => true,
            Asset::Cw20 { .. } => false,
        }
    }

    pub fn is_cw20(&self) -> bool {
        match self {
            Asset::Nft(_) => false,
            Asset::Cw20 { .. } => true,
        }
    }
}

#[cw_serde]
pub enum Status {
    Pending,
    Completed,
    Cancelled
}

#[cw_serde]
pub struct Order {
    pub id: u64,
    pub price: Coin,
    pub asset_class: Asset,
    pub seller: Addr,
    pub status: Status,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const ORDERS: Map<u64, Order> = Map::new("orders");
pub const ADMIN: Admin = Admin::new("admin");
pub const SENIOR_POOLS: Map<String, Addr> = Map::new("senior-pool-contracts");
