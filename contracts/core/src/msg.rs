use cosmwasm_std::{Addr, Decimal};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use crate::state;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{CosmosMsg, Empty};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub admins: Vec<String>,
    pub mutable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]

pub enum ExecuteMsg {
    CreateRequest{
        alias: String,
        address: Addr,
    },
    AcceptRequest{
        alias: String,
        address: Addr,
    },

    CreateProfile{
        name: String,
        address: String,
        jurisdiction: String,
        email_id: String,
        kyc_type: String,
    },
    // CreateInvoice{
    //     invoice_id: u64,
    //     amount: Decimal,
    //     currency: String,
    //     due_date: String,
    //     service_type: String,
    //     status: String,
    //     description: String,
    //     tax: Decimal,
    //     tax_percentage: Decimal,
    //     discount: Decimal,
    //     discount_percentage: Decimal,
    //     total: Decimal,
    //     contact_address: Addr,
    // },
    SetConfig {
        nft_address: Addr,
        owner: Addr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetInvoice {
        invoice_id: u64,
    },
    GetConfig {},
    GetLatestInvoiceId {},
    GetContactInfo {
        address: Addr,
    },

}
