#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Api, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdError,
    StdResult,
};

use crate::error::ContractError;
use crate::invoice::*;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::profile;
use crate::query::*;
use crate::state::*;
use cw2::set_contract_version;
// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    INVOICE_ID.save(deps.storage, &1000000)?;
    Ok(Response::default())
}

pub fn map_validate(api: &dyn Api, admins: &[String]) -> StdResult<Vec<Addr>> {
    admins.iter().map(|addr| api.addr_validate(addr)).collect()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    // Note: implement this function with different type to add support for custom messages
    // and then import the rest of this contract code.
    msg: ExecuteMsg,
) -> Result<Response<Empty>, ContractError> {
    match msg {
        ExecuteMsg::CreateRequest { address } => profile::create_request(deps, env, info, address),
        ExecuteMsg::AcceptRequest { address } => profile::accept_request(deps, env, info, address),
        ExecuteMsg::CreateProfile {
            name,
            email_id,
            phone_number,
            company_name,
            address,
        } => profile::create_profile(
            deps,
            env,
            info,
            name,
            email_id,
            phone_number,
            company_name,
            address,
        ),
        ExecuteMsg::SetConfig {
            nft_address,
            owner,
            accepted_assets,
        } => set_config(deps, nft_address, owner, accepted_assets),
        ExecuteMsg::CreateInvoice {
            address,
            receivable,
            amount_paid,
            service_type,
            doc_uri,
        } => create_invoice(
            deps,
            env,
            info,
            address,
            receivable,
            amount_paid,
            service_type,
            doc_uri,
        ),
        ExecuteMsg::AcceptInvoice { invoice_id } => accept_invoice(deps, env, info, invoice_id),
        ExecuteMsg::PayInvoice { invoice_id } => pay_invoice(deps, env, info, invoice_id),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetInvoice { invoice_id } => to_json_binary(&get_invoice(deps, invoice_id)?),
        QueryMsg::GetConfig {} => to_json_binary(&get_config(deps)?),
        QueryMsg::GetLatestInvoiceId {} => to_json_binary(&get_latest_invoice_id(deps)?),
        QueryMsg::GetContactInfo { address } => to_json_binary(&get_contact_info(deps, address)?),
        QueryMsg::GetPendingInvoices { address } => {
            to_json_binary(&get_pending_invoices(deps, address)?)
        }
        QueryMsg::GetExecutedInvoices { address } => {
            to_json_binary(&get_executed_invoices(deps, address)?)
        }
        QueryMsg::GetTotalReceivables { address } => {
            to_json_binary(&get_total_receivables(deps, address)?)
        }
        QueryMsg::GetTotalPayables { address } => to_json_binary(&get_total_payables(deps, address)?),
        QueryMsg::GetPendingContactRequests { address } => {
            to_json_binary(&get_pending_contact_requests(deps, address)?)
        }
        QueryMsg::GetSentContactRequests { address } => {
            to_json_binary(&get_sent_contact_requests(deps, address)?)
        }
        QueryMsg::GetAllContacts { address } => to_json_binary(&get_all_contacts(deps, address)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let ver = cw2::get_contract_version(deps.storage)?;
    // ensure we are migrating from an allowed contract
    if ver.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type").into());
    }
    // note: better to do proper semver compare, but string compare *usually* works
    if ver.version.as_str() > CONTRACT_VERSION {
        return Err(StdError::generic_err("Cannot upgrade from a newer version").into());
    }
    // set the new version
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    //do any desired state migrations...

    Ok(Response::default())
}
