// TODO
// * Deposit
//   - return lp_token
// * Invest


pub mod contract;
pub mod error;
pub mod msg;
pub mod state;

pub use crate::error::ContractError;
pub type ContractResult<T> = Result<T, ContractError>;
