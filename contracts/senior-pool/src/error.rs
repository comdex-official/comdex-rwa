use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Funds not allowed with this transaction")]
    FundsNotAllowed {},

    #[error("Funds required during execution")]
    ZeroFunds {},

    #[error("Multiple denoms not supported yet")]
    MultipleDenoms {},

    #[error("Required funds({required}) do not equal sent funds({sent})")]
    FundsMismatch { required: Uint128, sent: Uint128 },

}
