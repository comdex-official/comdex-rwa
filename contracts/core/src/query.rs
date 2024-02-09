use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::*;
use crate::invoice::*;
use crate::profile::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdResult,Coin
};


pub fn get_invoice(deps: Deps, invoice_id: u64) -> StdResult<Invoice> {
    let invoice = INVOICE.load(deps.storage, &invoice_id)?;
    Ok(invoice)
}

pub fn get_config(deps: Deps) -> StdResult<Config> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

pub fn get_latest_invoice_id(deps: Deps) -> StdResult<u64> {
    let id = INVOICE_ID.load(deps.storage)?;
    Ok(id)
}

pub fn get_contact_info(deps: Deps, address: Addr) -> StdResult<ContactInfo> {
    let contact_info = CONTACT_INFO.load(deps.storage, &address)?;
    Ok(contact_info)
}

pub fn get_pending_invoices(deps: Deps, address: Addr) -> StdResult<(Vec<Invoice>,Vec<Invoice>)> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok((vec![],vec![]));
    }
    let contact_info = contact_info.unwrap();
    let mut sent_invoices=vec![];
    let mut received_invoices=vec![];
    for invoice_id in contact_info.generated_invoices.iter() {
        let invoice = INVOICE.may_load(deps.storage, &invoice_id)?;
        if invoice.is_none() {
            continue;
        }
        let invoice = invoice.unwrap();
        if invoice.status!=Status::Paid{
            sent_invoices.push(invoice);
        }
    }
    for invoice_id in contact_info.assigned_invoices.iter() {
        let invoice = INVOICE.may_load(deps.storage, &invoice_id)?;
        if invoice.is_none() {
            continue;
        }
        let invoice = invoice.unwrap();
        if invoice.status!=Status::Paid{
            received_invoices.push(invoice);
        }
    }
    Ok((sent_invoices,received_invoices))
}

pub fn get_executed_invoices(deps: Deps, address: Addr) -> StdResult<(Vec<Invoice>,Vec<Invoice>)> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok((vec![],vec![]));
    }
    let contact_info = contact_info.unwrap();
    let mut sent_invoices=vec![];
    let mut received_invoices=vec![];
    for invoice_id in contact_info.generated_invoices.iter() {
        let invoice = INVOICE.may_load(deps.storage, &invoice_id)?;
        if invoice.is_none() {
            continue;
        }
        let invoice = invoice.unwrap();
        if invoice.status==Status::Paid{
            sent_invoices.push(invoice);
        }
    }
    for invoice_id in contact_info.assigned_invoices.iter() {
        let invoice = INVOICE.may_load(deps.storage, &invoice_id)?;
        if invoice.is_none() {
            continue;
        }
        let invoice = invoice.unwrap();
        if invoice.status==Status::Paid{
            received_invoices.push(invoice);
        }
    }
    Ok((sent_invoices,received_invoices))
}

pub fn get_total_receivables(deps: Deps, address: Addr) -> StdResult<Vec<Coin>> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok(vec![]);
    }
    let contact_info = contact_info.unwrap();
    let mut total_receivables: Vec<Coin>=vec![];
    for invoice_id in contact_info.generated_invoices.iter() {
        let invoice = INVOICE.may_load(deps.storage, &invoice_id)?;
        if invoice.is_none() {
            continue;
        }
        let invoice = invoice.unwrap();

        if invoice.status!=Status::Paid{
            //// if total_receivables is umpty push it but if not empty , update the amount if denom matches
            let mut found=false;
            for receivable in total_receivables.iter_mut() {
                if receivable.denom==invoice.receivable.denom {
                    receivable.amount+=invoice.receivable.amount-invoice.amount_paid.amount;
                    found=true;
                    break;
                }
            }
            if !found {
                total_receivables.push(Coin{
                    denom:invoice.receivable.denom.clone(),
                    amount:invoice.receivable.amount-invoice.amount_paid.amount,
                });
            }
        }
    }
    Ok(total_receivables)
}

pub fn get_total_payables(deps: Deps, address: Addr) -> StdResult<Vec<Coin>> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok(vec![]);
    }
    let contact_info = contact_info.unwrap();

    let mut total_payables: Vec<Coin>=vec![];
    for invoice_id in contact_info.assigned_invoices.iter() {
        let invoice = INVOICE.may_load(deps.storage, &invoice_id)?;
        if invoice.is_none() {
            continue;
        }
        let invoice = invoice.unwrap();
        if invoice.status!=Status::Paid{
            //// if total_payables is umpty push it but if not empty , update the amount if denom matches
            let mut found=false;
            for payable in total_payables.iter_mut() {
                if payable.denom==invoice.receivable.denom {
                    payable.amount+=invoice.receivable.amount-invoice.amount_paid.amount;
                    found=true;
                    break;
                }
            }
            if !found {
                total_payables.push(Coin{
                    denom:invoice.receivable.denom.clone(),
                    amount:invoice.receivable.amount-invoice.amount_paid.amount,
                });
            }
        }
    }
    Ok(total_payables)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ContactResponse {
    pub name: String,
    pub address: Addr,
    pub company_name: String,
}

pub fn get_pending_contact_requests(deps: Deps, address: Addr) -> StdResult<Vec<ContactResponse>> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok(vec![]);
    }
    let contact_info = contact_info.unwrap();

    let received_requests=contact_info.received_requests.clone();
    let mut response=vec![];
    for contact in received_requests.iter() {
        let contact_info = CONTACT_INFO.load(deps.storage, &contact)?;
        response.push(ContactResponse {
            name: contact_info.name,
            address: contact_info.owner,
            company_name: contact_info.company_name,
        });
    }
    Ok(response)
}

pub fn get_sent_contact_requests(deps: Deps, address: Addr) -> StdResult<Vec<ContactResponse>> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok(vec![]);
    }
    let contact_info = contact_info.unwrap();
    let sent_requests=contact_info.sent_requests.clone();
    let mut response=vec![];
    for contact in sent_requests.iter() {
        let contact_info = CONTACT_INFO.load(deps.storage, &contact)?;
        response.push(ContactResponse {
            name: contact_info.name,
            address: contact_info.owner,
            company_name: contact_info.company_name,
        });
    }
    Ok(response)
}

pub fn get_all_contacts(deps: Deps, address: Addr) -> StdResult<Vec<ContactResponse>> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        return Ok(vec![]);
    }
    let contact_info = contact_info.unwrap();
    let contacts=contact_info.contacts.clone();
    let mut response=vec![];
    for contact in contacts.iter() {
        let contact_info = CONTACT_INFO.load(deps.storage, &contact)?;
        response.push(ContactResponse {
            name: contact_info.name,
            address: contact_info.owner,
            company_name: contact_info.company_name,
        });
    }
    Ok(response)
}




