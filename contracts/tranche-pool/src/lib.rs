// !-------
// REMOVE THIS LINE
// -------!
#![allow(unused_variables, unused_mut, unused_assignments)]
pub mod contract;
pub mod error;
pub mod msg;
pub mod state;
pub mod credit_line;
pub mod helpers;
pub mod query;
pub mod tranche_pool;
pub mod implementation;

use cosmwasm_std::Uint128;

pub use crate::error::ContractError;

pub const TEN_THOUSAND: Uint128 = Uint128::new(10000u128);
pub const SIY: Uint128 = Uint128::new(365u128 * 24u128 * 3600u128);
pub const GRACE_PERIOD: u64 = 24u64 * 3600u64 * 7;
