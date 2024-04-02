use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Uint128,Addr};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub cw20_code_id: u64,
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
