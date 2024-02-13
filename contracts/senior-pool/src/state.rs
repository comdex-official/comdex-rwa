use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Timestamp, Uint128};
use cw_storage_plus::{Item};
use cw_controllers::Admin;

use crate::ContractResult;

#[cw_serde]
pub struct Config {
    pub lp_token: String,
}

#[cw_serde]
pub struct Epoch {
    pub end_time: Timestamp,
    pub lp_token_requested: Uint128,
    pub lp_token_liquidated: Uint128,
    pub usdc_allocated: Uint128,
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

pub const ADMIN: Admin = Admin::new("admin");
pub const CONFIG: Item<Config> = Item::new("config");
