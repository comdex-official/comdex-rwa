use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;
use cw721_base::ExecuteMsg as Cw721BaseExecuteMsg;

#[cw_serde]
pub enum ExecuteMsg<MintMsg, ExecuteExt> {
    WithdrawPrincipal {
        token_id: String,
        principal_amount: Uint128,
    },
    Redeem {
        token_id: String,
        principal_redeemed: Uint128,
        interest_redeemed: Uint128,
    },
    Base(Cw721BaseExecuteMsg<MintMsg, ExecuteExt>),
}
