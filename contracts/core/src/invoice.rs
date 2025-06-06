use crate::msg::{InstantiateMsg, QueryMsg};
use crate::state::*;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Api, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdError, Uint128, WasmMsg,
};
use cw721_base::msg::{ExecuteMsg, MintMsg};

use crate::error::ContractError;

pub fn create_invoice(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    payee_address: Addr,
    receivable: Coin,
    amount_paid: Coin,
    service_type: ServiceType,
    doc_uri: String,
) -> Result<Response, ContractError> {
    //// do not accept funds ////
    if !info.funds.is_empty() {
        return Err(StdError::generic_err("Funds not accepted").into());
    }

    //// Address cannot be sender////
    if info.sender == payee_address {
        return Err(StdError::generic_err("Receiver and Sender cannot be same").into());
    }

    //// check if denom of receivable and amount_paid and is same ////
    if receivable.denom != amount_paid.denom {
        return Err(
            StdError::generic_err("Denom of receivable and amount_paid should be same").into(),
        );
    }

    //// iterate config accepted asset to check if receivable denom is accepted ////

    let config = CONFIG.load(deps.storage)?;
 
    if !config.accepted_assets.iter().any(|asset| asset.denom == receivable.denom) {
        return Err(StdError::generic_err("Asset not accepted").into());
    }

        // Load and validate sender's contact info
        let mut sender_contact_info = CONTACT_INFO.may_load(deps.storage, &info.sender)?
        .ok_or_else(|| StdError::generic_err("Profile does not exist"))?;

 
    if sender_contact_info.kyc_status != KYCStatus::Approved {
        return Err(StdError::generic_err("Creator KYC not verified").into());
    }

    //// if address doesnt exists in contact _info.contact throw error
    let contacts = sender_contact_info.contacts.clone();
    if !contacts.contains(&payee_address) {
        return Err(StdError::generic_err("Receiver not in contact list").into());
    }
    
    //// Check if counter party is verified or not
    let mut receiver_contact_info = CONTACT_INFO.may_load(deps.storage, &payee_address)?
    .ok_or_else(|| StdError::generic_err("CounterParty Profile does not exist"))?;

    if receiver_contact_info.kyc_status == KYCStatus::Unverified {
        return Err(StdError::generic_err("CounterParty KYC not verified").into());
    }

    let due_amount=Coin {
        denom: receivable.denom.clone(),
        amount: receivable.amount - amount_paid.amount,
    };

    let invoice_id = get_invoice_id(deps.as_ref());
    let invoice = Invoice {
        id: invoice_id,
        from: info.sender.clone(),
        payee_address: payee_address.clone(),
        nft_id: invoice_id ,
        doc_uri: doc_uri.clone(),
        due_amount: due_amount,
        receivable: receivable.clone(),
        amount_paid: Coin {
            denom: receivable.denom.clone(),
            amount: Uint128::zero(),
        },
        service_type: service_type.clone(),
        status: Status::Raised,
    };

    INVOICE.save(deps.storage, &invoice_id, &invoice)?;

    sender_contact_info.generated_invoices.push(invoice_id);
    CONTACT_INFO.save(deps.storage, &info.sender, &sender_contact_info)?;

    ///// updated assigned invoice list
    receiver_contact_info.assigned_invoices.push(invoice_id);
    CONTACT_INFO.save(deps.storage, &payee_address, &receiver_contact_info)?;

    let metadata = Metadata {
        invoice_id: invoice_id,
        from: info.sender.clone(),
        payee_address: payee_address.clone(),
        receivable: receivable.clone(),
        uri: doc_uri.clone(),
    };

    let mint_msg = MintMsg {
        token_id: invoice_id.to_string(),
        owner: env.contract.address.to_string(),
        token_uri: None,
        extension: metadata,
    };

    let msg: ExecuteMsg<Metadata, Empty> = ExecuteMsg::Mint(mint_msg);

    let message: CosmosMsg<_> = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.nft_address.into_string(),
        msg: to_binary(&msg)?,
        funds: vec![],
    });

    INVOICE_ID.save(deps.storage, &(invoice_id + 1))?;

    Ok(Response::new()
        .add_message(message)
        .add_attribute("invoice_id", invoice_id.to_string()))
}

pub fn pay_invoice(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    invoice_id: u64,
) -> Result<Response, ContractError> {
    // Validate that exactly one type of token is sent
    let funds = match info.funds.as_slice() {
        [fund] => fund,
        _ => return Err(StdError::generic_err("Accepts only one token").into()),
    };


    //// iterate config accepted asset to check if receivable denom is accepted ////

    let config = CONFIG.load(deps.storage)?;
    if !config.accepted_assets.iter().any(|asset| asset.denom == funds.denom) {
        return Err(StdError::generic_err("Token not accepted").into());
    }

    let mut invoice = INVOICE.load(deps.storage, &invoice_id)?;

    if invoice.payee_address != info.sender {
        return Err(StdError::generic_err("Receiver and Sender cannot be same").into());
    }

    // Check the invoice status
    match invoice.status {
        Status::Raised => return Err(StdError::generic_err("Invoice not yet accepted").into()),
        Status::Paid => return Err(StdError::generic_err("Invoice already paid").into()),
        _ => (),
    }

    let amount = info.funds[0].amount;
    let denom = info.funds[0].denom.clone();

    let amount_paid = invoice.amount_paid.amount.clone();
    let receivable = invoice.receivable.amount.clone();

    if amount_paid + amount > receivable {
        return Err(StdError::generic_err("Amount paid exceeds receivable").into());
    }

    let mut response: Response<Empty> =
        Response::new().add_attribute("invoice_id", invoice_id.to_string());
    if amount_paid + amount == receivable {
        invoice.amount_paid.amount = invoice.amount_paid.amount + info.funds[0].amount.clone();
        invoice.status = Status::Paid;
        invoice.due_amount.amount = Uint128::zero();
        INVOICE.save(deps.storage, &invoice_id, &invoice)?;

        //// transfer nft to owner ////
        let msg: ExecuteMsg<Empty, Empty> = ExecuteMsg::TransferNft {
            recipient: invoice.from.to_string(),
            token_id: invoice_id.to_string(),
        };

        let message: CosmosMsg<Empty> = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.nft_address.into_string(),
            msg: to_binary(&msg)?,
            funds: vec![],
        });


        response = response.add_message(message);
    } else {
        invoice.amount_paid.amount = invoice.amount_paid.amount + info.funds[0].amount.clone();
        invoice.due_amount.amount = receivable - (amount_paid + amount);
        invoice.status = Status::PartiallyPaid;
        INVOICE.save(deps.storage, &invoice_id, &invoice)?;
    }

    let bank_msg: CosmosMsg<Empty> = CosmosMsg::Bank(BankMsg::Send {
        to_address: invoice.from.to_string(),
        amount: vec![Coin {
            denom: denom.clone(),
            amount: amount,
        }],
    });

    response = response.add_message(bank_msg);

    Ok(response
        .add_attribute("method", "pay_invoice")
        .add_attribute("amount", amount.to_string())
        .add_attribute("denom", denom))
}

pub fn accept_invoice(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    invoice_id: u64,
) -> Result<Response, ContractError> {
    let mut invoice = INVOICE.load(deps.storage, &invoice_id)?;

    if invoice.payee_address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    if invoice.status != Status::Raised {
        return Err(StdError::generic_err("Invoice already accepted").into());
    }

    invoice.status = Status::Accepted;
    INVOICE.save(deps.storage, &invoice_id, &invoice)?;

    Ok(Response::new()
        .add_attribute("method", "accept_invoice")
        .add_attribute("invoice_id", invoice_id.to_string()))
}
