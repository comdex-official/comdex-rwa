use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Timestamp, Uint128,Addr};
use cw_storage_plus::{Item,Map};
use cw_controllers::Admin;

use crate::ContractResult;

#[cw_serde]
pub struct Config {
    pub lp_token: String,
    pub pool_denom: String,
    pub max_leverage_ratio: Decimal,
}

#[cw_serde]
pub struct FundInfo {
    /// Amount available in senior pool
    pub available: Uint128,
    /// Amount invested in junior pools
    pub invested: Uint128,
    pub total_loans_outstanding: Uint128,
    pub total_writedowns: Uint128,
    pub share_price: Decimal,
}

#[cw_serde]
pub struct Epoch {
    pub end_time: Timestamp,
    pub lp_token_requested: Uint128,
    pub lp_token_liquidated: Uint128,
    pub usdc_allocated: Uint128,
}

#[cw_serde]
pub struct Deposits {
   pub deposited: Uint128,
   pub deposit_epoch: u64,
}


pub trait SeniorPool {
    fn deposit(amount: Uint128) -> ContractResult<()>;
    fn withdraw(amount: Uint128) -> ContractResult<()>;
    fn invest(pool_id: u64) -> ContractResult<Uint128>;
    fn estimate_investment(pool_id: u64) -> ContractResult<Uint128>;
    fn writedown(token_id: Uint128) -> ContractResult<()>;
}

pub trait InvestmentStrategy {
    fn get_leverage_ratio(pool_id: u64) -> ContractResult<Uint128>;
    fn invest(pool_id: u64) -> ContractResult<Uint128>;
    fn estimate_investment(pool_id: u64) -> ContractResult<Uint128>;
}

pub const CHECK_POINTED_EPOCH_ID: Item<u64> = Item::new("checkpointed_epoch");
pub const EPOCH_DURATION: Item<u64> = Item::new("epoch_duration");
pub const FUND_INFO: Item<FundInfo> = Item::new("senior_pool_funds");
pub const ADMIN: Admin = Admin::new("admin");
pub const CONFIG: Item<Config> = Item::new("config");
pub const WRITE_DOWNS_BY_POOL_AMOUNT: Map<u64,Uint128> = Map::new("write_downs_by_pool_amount");
pub const EPOCH :Map<u64,Epoch> = Map::new("epoch");
pub const USER_DEPOSIT: Map<Addr,Deposits> = Map::new("user_deposit");