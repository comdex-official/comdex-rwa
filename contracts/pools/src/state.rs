use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Env, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cw_serde]
pub struct Config {
    pub pool_id: u64,
    pub grace_period: Option<BlockInfo>,
}

#[cw_serde]
pub struct InvestorTotem {
    pub token_id: u128,
    pub pool_id: u64,
    pub lend_info: LendInfo,
}

#[cw_serde]
pub struct TranchePool {
    pub pool_id: u64,
    pub borrow_limit: Uint128,
    pub interest_apy: u16,
    pub borrower_addr: Addr,
    pub creation_info: BlockInfo,
    pub payment_schedule: PaymentSchedule,
    pub drawdown_info: Option<BlockInfo>,
    pub grace_period: Option<BlockInfo>,
    pub junior_tranche: LendInfo,
    pub senior_tranche: LendInfo,
    pub backers: Vec<u128>,
}

impl TranchePool {
    pub fn new(
        pool_id: u64,
        borrow_limit: Uint128,
        interest_apy: u16,
        borrower: Addr,
        payment_schedule: PaymentSchedule,
        env: &Env,
    ) -> Self {
        TranchePool {
            pool_id,
            borrow_limit,
            interest_apy,
            borrower_addr: borrower,
            creation_info: BlockInfo::new(env),
            payment_schedule,
            drawdown_info: None,
            grace_period: None,
            junior_tranche: LendInfo::default(),
            senior_tranche: LendInfo::default(),
            backers: Vec::new(),
        }
    }

    pub fn set_grace_period(&self, new_grace_period: BlockInfo) {
        self.grace_period = Some(new_grace_period);
    }
}

#[cw_serde]
pub struct LendInfo {
    pub principal_deposited: Uint128,
    pub principal_redeemed: Uint128,
    pub interest_redeemed: Uint128,
}

impl Default for LendInfo {
    fn default() -> Self {
        LendInfo {
            principal_deposited: Uint128::zero(),
            principal_redeemed: Uint128::zero(),
            interest_redeemed: Uint128::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentSchedule {
    Monthly,
    Quaterly,
    Biannually,
    Annually,
}

#[cw_serde]
pub struct BlockInfo {
    pub height: u64,
    pub timestamp: Timestamp,
}

impl BlockInfo {
    pub fn new(env: &Env) -> Self {
        BlockInfo {
            height: env.block.height,
            timestamp: env.block.time,
        }
    }
}

pub const CONFIG: Item<Config> = Item::new("pool_config");
pub const TRANCHE_POOLS: Map<u64, TranchePool> = Map::new("tranche_pools");
