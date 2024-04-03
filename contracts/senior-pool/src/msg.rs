use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub cw20_code_id: u64,
    pub nft_token_addr: String,
    pub pool_denom: String,
    pub max_leverage_ratio: Decimal,
    pub junior_pools: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit(DepositMsg),
    Withdraw { amount: Uint128 },
    Invest { pool_id: u64 ,tranche_id: u64, amount: Uint128 },    
}

#[cw_serde]
pub enum QueryMsg {
    GetUserDeposits { address: Addr },
    GetConfig {},
    GetFundInfo {},
    MaxLeverageRatio {},
    

}

#[cw_serde]
pub struct DepositMsg {
    pub amount: Uint128,
}
