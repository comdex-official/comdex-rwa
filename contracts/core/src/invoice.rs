use crate::msg::{ InstantiateMsg, QueryMsg};
use crate::state::*;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, StdResult,WasmMsg,SubMsg,
};

use cw721_base::msg::ExecuteMsg;
use cw721_base::msg::MintMsg;
use crate::error::ContractError;



pub fn create_invoice(
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

        return Err(StdError::generic_err("Profile does not exist").into());

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
    let new_contact = Contact {
        alias: alias.clone(),
        contact_address: address.clone(),
    };
    contact_info.sent_requests.push(new_contact.clone());
    CONTACT_INFO.save(deps.storage, &info.sender, &contact_info)?;

    //// check if address exist in contact_info.received_requests ////
    let mut contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    if contact_info.is_none() {
        let new_contact_info = ContactInfo {
            sent_requests: vec![],
            received_requests: vec![],
            contacts:
                vec![Contact {
                    alias: alias.clone(),
                    contact_address: info.sender.clone(),
                }], // add sender to contacts
            account_address: address.clone(),
            name: "".to_string(),
            address: "".to_string(),
            jurisdiction: "".to_string(),
            owner: address.clone(),
            email_id: "".to_string(),
            kyc_type: "".to_string(),
            kyc_status: KYCStatus::Unverified,
            assigned_invoices: vec![],
            generated_invoices: vec![],

        };
        CONTACT_INFO.save(deps.storage, &address, &new_contact_info)?;
        contact_info = CONTACT_INFO.may_load(deps.storage, &address)?;
    }
    let mut contact_info = contact_info.unwrap();

    let mint_msg=MintMsg {
        token_id: "1".to_string(),
        owner: info.sender.clone().to_string(),
        token_uri: None,
        extension: contact_info.clone(),
    };

    
    let msg: ExecuteMsg<ContactInfo, Empty>=ExecuteMsg::Mint(mint_msg);

    let message: SubMsg<Empty> = SubMsg::new(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&msg)?,
        funds: vec![],
    });


    Ok(Response::new().add_submessage(message))
}
