use crate::state::*;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal};
use cosmwasm_std::{Coin, CosmosMsg, Empty};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub admins: Vec<String>,
    pub mutable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]

pub enum ExecuteMsg {
    CreateRequest {
        address: Addr,
    },
    AcceptRequest {
        address: Addr,
    },

    CreateProfile {
        name: String,
        email_id: String,
        phone_number: String,
        company_name: String,
        address: String,
    },
    CreateInvoice {
        payee_address: Addr,
        receivable: Coin,
        amount_paid: Coin,
        service_type: ServiceType,
        doc_uri: String,
    },

    SetConfig {
        nft_address: Addr,
        owner: Addr,
        accepted_assets: Vec<Asset>,
    },
    AcceptInvoice {
        invoice_id: u64,
    },
    PayInvoice {
        invoice_id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetInvoice { invoice_id: u64 },
    GetConfig {},
    GetLatestInvoiceId {},
    GetContactInfo { address: Addr },
    GetPendingInvoices { address: Addr },
    GetExecutedInvoices { address: Addr },
    GetTotalReceivables { address: Addr },
    GetTotalPayables { address: Addr },
    GetPendingContactRequests { address: Addr },
    GetSentContactRequests { address: Addr },
    GetAllContacts { address: Addr },
}
