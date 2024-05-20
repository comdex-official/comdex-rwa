use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub nft_contract: String,
    pub admin: String,
    // Tuple of (denom, contract addr) pairs
    pub senior_pools: Vec<(String, String)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Executed when listing a NFT asset.
    NftSellOrder { token_id: String, price: Coin },
    /// Executed when listing a Cw20 asset.
    TokenSellOrder { amount: Uint128, denom: String, price: Coin },
    /// Executed when buying a listed asset.
    BuyOrder { order_id: u64 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
