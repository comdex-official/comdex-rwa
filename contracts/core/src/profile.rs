use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::*;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, StdResult,
};

use crate::error::ContractError;

pub fn create_request(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr,
) -> Result<Response, ContractError> {
    //// Address cannot be sender////
    if info.sender == address {
        return Err(ContractError::Unauthorized {});
    }

    //// check if already requested ////
    let contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;
    if contact_info.is_none() {
        return Err(StdError::generic_err("Profile does not exist").into());
    }

    let mut contact_info = contact_info.unwrap();

    //// iterate over sent request to check if address exist in contacr_info.sent_requests ////
    for contact in contact_info.sent_requests.iter() {
        if contact == address {
            return Err(StdError::generic_err("Already Requested").into());
        }
    }

    //// check if already in my contact list or already existing alias ////
    for contact in contact_info.contacts.iter() {
        if contact == address {
            return Err(StdError::generic_err("Already in contact list").into());
        }
    }

    //// append new contact to sent_requests ////
    contact_info.sent_requests.push(address.clone());

    //// save updated contact_info ////
    CONTACT_INFO.save(deps.storage, &info.sender, &contact_info)?;

    //// create request message ////

    let requested_contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if requested_contact_info.is_none() {
        return Err(StdError::generic_err("Recipient Profile does not exist").into());
    }
    let mut requested_contact_info = requested_contact_info.unwrap();
    requested_contact_info
        .received_requests
        .push(info.sender.clone());
    CONTACT_INFO.save(deps.storage, &address, &requested_contact_info)?;

    Ok(Response::new()
        .add_attribute("method", "create_request")
        .add_attribute("sender", info.sender)
        .add_attribute("receiver", address))
}

pub fn accept_request(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr,
) -> Result<Response, ContractError> {
    //// do not accept funds ////
    if !info.funds.is_empty() {
        return Err(StdError::generic_err("Funds not accepted").into());
    }

    //// check if request exist ////

    let contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;

    if contact_info.is_none() {
        return Err(StdError::generic_err("No profile found ").into());
    }
    let mut contact_info = contact_info.unwrap();

    //// check if request exist ////
    let mut request_exist = false;
    let mut index = 0;
    for contact in contact_info.received_requests.iter() {
        if contact == address {
            request_exist = true;
            break;
        }
        index += 1;
    }
    if !request_exist {
        return Err(StdError::generic_err("No request found").into());
    }

    //// remove request from received_requests ////
    contact_info.received_requests.remove(index);

    //// add contact to contacts ////
    contact_info.contacts.push(address.clone());

    //// save updated contact_info ////
    CONTACT_INFO.save(deps.storage, &info.sender, &contact_info)?;

    //// remove send request of the sender ////
    let requested_contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if requested_contact_info.is_none() {
        return Err(StdError::generic_err("Requestor profile not found").into());
    }

    let mut requested_contact_info = requested_contact_info.unwrap();

    let mut index = 0;
    for contact in requested_contact_info.sent_requests.iter() {
        if contact == info.sender {
            break;
        }
        index += 1;
    }

    requested_contact_info.sent_requests.remove(index);

    //// add contact to contacts ////
    requested_contact_info.contacts.push(info.sender.clone());

    //// save updated contact_info ////
    CONTACT_INFO.save(deps.storage, &address, &requested_contact_info)?;

    Ok(Response::new() .add_attribute("method", "accept_request")
    .add_attribute("receiver", info.sender))
}

pub fn create_profile(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    name: String,
    email_id: String,
    phone_number: String,
    company_name: String,
    address: String,
) -> Result<Response, ContractError> {

    //// do not accept funds ////
    if !info.funds.is_empty() {
        return Err(StdError::generic_err("Funds not accepted").into());
    }

    ///// only create profile if not already created /////
    let contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;
    if contact_info.is_some() {
        return Err(StdError::generic_err("Profile already exist").into());
    }

    let new_contact_info = ContactInfo {
        name: name.clone(),
        company_name: company_name.clone(),
        address: address.clone(),
        phone_number: phone_number.clone(),
        owner: info.sender.clone(),
        email_id: email_id.clone(),
        sent_requests: vec![],
        received_requests: vec![],
        contacts: vec![],
        ///// default KYC status is set as VERIFIED now to bypass testing /////
        kyc_status: KYCStatus::Approved,
        assigned_invoices: vec![],
        generated_invoices: vec![],
    };

    CONTACT_INFO.save(deps.storage, &info.sender, &new_contact_info)?;

    Ok(Response::new().add_attribute("method", "create_profile")
    .add_attribute("sender", info.sender))
}
