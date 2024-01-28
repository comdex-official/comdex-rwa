use cosmwasm_std::Deps;

use crate::error::ContractResult;
use crate::{state::CONFIG, GRACE_PERIOD};

pub fn get_grace_period(deps: Deps) -> ContractResult<u64> {
    let config = CONFIG.load(deps.storage)?;

    Ok(config.grace_period.unwrap_or(GRACE_PERIOD))
}
