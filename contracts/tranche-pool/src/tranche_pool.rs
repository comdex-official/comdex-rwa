use crate::state::{PoolType, TranchePool};
use cosmwasm_std::{Addr, Env};

impl TranchePool {
    pub fn new(
        pool_id: u64,
        pool_name: String,
        borrower: Addr,
        borrower_name: String,
        denom: String,
        backers: Vec<Addr>,
        pool_type: PoolType,
        env: &Env,
    ) -> Self {
        TranchePool {
            pool_id,
            pool_name,
            borrower_addr: borrower,
            borrower_name,
            creation_info: env.block.time,
            denom,
            backers,
            pool_type,
        }
    }

    pub fn is_backer(&self, user: &Addr) -> bool {
        self.backers.iter().any(|backer| backer == user)
    }
}
