use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::*;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, StdResult,
};

use crate::error::ContractError;

pub fn create_request(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    alias: String,
    address: Addr,
) -> Result<Response, ContractError> {
    //// Address cannot be sender////
    if info.sender == address {
        return Err(ContractError::Unauthorized {});
    }

    //// check if already requested ////
    let mut contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;
    if contact_info.is_none() {
        let new_contact_info = ContactInfo {
            sent_requests: vec![],
            received_requests: vec![],
            contacts: vec![],
            account_address: address.clone(),
            name: "".to_string(),
            address: "".to_string(),
            jurisdiction: "".to_string(),
            owner: info.sender.clone(),
            email_id: "".to_string(),
            kyc_type: "".to_string(),
            kyc_status: KYCStatus::Unverified,
            assigned_invoices: vec![],
            generated_invoices: vec![],
        };
        CONTACT_INFO.save(deps.storage, &info.sender, &new_contact_info)?;
        contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;
    }
    let mut contact_info = contact_info.unwrap();

    //// iterate over sent request to check if address exist in contacr_info.sent_requests ////
    for contact in contact_info.sent_requests.iter() {
        if contact.contact_address == address {
            return Err(StdError::generic_err("Already Requested").into());
        }
    }

    //// check if already in my contact list or already existing alias ////
    for contact in contact_info.contacts.iter() {
        if contact.contact_address == address {
            return Err(StdError::generic_err("Already in contact list").into());
        }
        if contact.alias == alias {
            return Err(StdError::generic_err("Alias already exist").into());
        }
    }

    //// append new contact to sent_requests ////
    contact_info.sent_requests.push(Contact {
        alias: alias.clone(),
        contact_address: address.clone(),
    });

    //// save updated contact_info ////
    CONTACT_INFO.save(deps.storage, &info.sender, &contact_info)?;

    //// create request message ////

    let mut requested_contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if requested_contact_info.is_none() {
        let new_contact_info = ContactInfo {
            sent_requests: vec![],
            received_requests: vec![],
            contacts: vec![],
            account_address: address.clone(),
            name: "".to_string(),
            address: "".to_string(),
            jurisdiction: "".to_string(),
            owner: info.sender.clone(),
            email_id: "".to_string(),
            kyc_type: "".to_string(),
            kyc_status: KYCStatus::Unverified,
            assigned_invoices: vec![],
            generated_invoices: vec![],

        };
        CONTACT_INFO.save(deps.storage, &address, &new_contact_info)?;
        requested_contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    }
    let mut requested_contact_info = requested_contact_info.unwrap();
    requested_contact_info.received_requests.push(Contact {
        alias: alias.clone(),
        contact_address: info.sender.clone(),
    });
    CONTACT_INFO.save(deps.storage, &address, &requested_contact_info)?;

    Ok(Response::new())
}

pub fn accept_request(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr,
    alias: String,
) -> Result<Response, ContractError> {
    //// check if request exist ////

    let contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;

    if contact_info.is_none() {
        return Err(StdError::generic_err("No request found").into());
    }
    let mut contact_info = contact_info.unwrap();

    //// check if request exist ////
    let mut request_exist = false;
    let mut index = 0;
    for contact in contact_info.received_requests.iter() {
        if contact.contact_address == address {
            request_exist = true;
            break;
        }
        index += 1;
    }
    if !request_exist {
        return Err(StdError::generic_err("No request found").into());
    }

    //// check if alias already exist ////
    for contact in contact_info.contacts.iter() {
        if contact.alias == alias {
            return Err(StdError::generic_err("Alias already exist").into());
        }
    }

    //// remove request from received_requests ////
    contact_info.received_requests.remove(index);

    //// add contact to contacts ////
    contact_info.contacts.push(Contact {
        alias: alias.clone(),
        contact_address: address.clone(),
    });

    //// save updated contact_info ////
    CONTACT_INFO.save(deps.storage, &info.sender, &contact_info)?;

    //// remove send request of the sender ////
    let requested_contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if requested_contact_info.is_none() {
        return Err(StdError::generic_err("No request found").into());
    }

    let mut requested_contact_info = requested_contact_info.unwrap();

    let mut index = 0;
    for contact in requested_contact_info.sent_requests.iter() {
        if contact.contact_address == info.sender {
            break;
        }
        index += 1;
    }

    requested_contact_info.sent_requests.remove(index);

    //// add contact to contacts ////
    requested_contact_info.contacts.push(Contact {
        alias: alias.clone(),
        contact_address: info.sender.clone(),
    });

    //// save updated contact_info ////
    CONTACT_INFO.save(deps.storage, &address, &requested_contact_info)?;

    Ok(Response::new())
}

pub fn create_profile(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    name: String,
    address: String,
    jurisdiction: String,
    email_id: String,
    kyc_type: String,
) -> Result<Response, ContractError> {
    ///// only create profile if not already created /////
    let contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;
    if contact_info.is_some() {
        return Err(StdError::generic_err("Profile already exist").into());
    }

    let new_contact_info = ContactInfo {
        sent_requests: vec![],
        received_requests: vec![],
        contacts: vec![],
        account_address: info.sender.clone(),
        name: name.clone(),
        address: address.clone(),
        jurisdiction: jurisdiction.clone(),
        owner: info.sender.clone(),
        email_id: email_id.clone(),
        kyc_type: kyc_type.clone(),
        kyc_status: KYCStatus::Unverified,
        assigned_invoices: vec![],
        generated_invoices: vec![],
    };
    
    CONTACT_INFO.save(deps.storage, &info.sender, &new_contact_info)?;

    Ok(Response::new())
}

pub fn update_kyc(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    kyc_type: String,
    kyc_status: KYCStatus,
) -> Result<Response, ContractError> {
    let contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?;
    if contact_info.is_none() {
        return Err(StdError::generic_err("No profile found").into());
    }
    let mut contact_info = contact_info.unwrap();

    contact_info.kyc_type = kyc_type.clone();
    contact_info.kyc_status = kyc_status.clone();

    CONTACT_INFO.save(deps.storage, &info.sender, &contact_info)?;

    Ok(Response::new())
}
