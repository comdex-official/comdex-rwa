use cosmwasm_std::{StdError, Uint128, OverflowError, DivideByZeroError};
use thiserror::Error;

pub type ContractResult<T> = Result<T, ContractError>;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),

    #[error("Invalid address: {address}")]
    InvalidAdmin { address: String },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Transaction does not accept funds")]
    FundsNotAllowed,

    #[error("Transaction requires funds during execution")]
    EmptyFunds,

    #[error("Single token denomination allowed")]
    MultipleTokens,

    #[error("Required funds({required}) doesn't equal sent funds({sent})")]
    FundDiscrepancy { required: Uint128, sent: Uint128 },

    #[error("{id} is not a valid Pool ID")]
    InvalidPoolId { id: u64 },

    #[error("Drawdown amount exceeds total limit({limit})")]
    DrawdownExceedsLimit { limit: Uint128 },

}
