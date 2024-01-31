use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin,DepsMut,Deps,Response,StdError,StdResult};
use cw_storage_plus::{Item, Map};
use crate::error::ContractError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub nft_address: Addr,
    pub owner: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Contact {
    pub alias: String,
    pub contact_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ContactInfo {
    pub account_address: Addr,
    pub name: String,
    pub address: String,
    pub jurisdiction: String,
    pub owner: Addr,
    pub email_id: String,
    pub sent_requests: Vec<Contact>,
    pub received_requests: Vec<Contact>,
    pub contacts: Vec<Contact>,
    pub kyc_type: String,
    pub kyc_status: KYCStatus,
    pub assigned_invoices: Vec<u64>,
    pub generated_invoices: Vec<u64>,
}

pub const CONTACT_INFO: Map<&Addr, ContactInfo> = Map::new("contact_info");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]

pub enum ServiceType {
    Unspecified,
    Goods,
    Service,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Unspecified,
    Raised,
    Accepted,
    Paid,
    PartiallyPaid,
    ReVerify,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KYCStatus {
    Unverified,
    InProgess,
    Rejected,
    Approved,
    ReVerify,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Invoice {
    pub id: u8,
    pub from: Addr,
    pub receiver: Addr,
    pub counter_party_id: u8,
    pub nft_id: u8,
    pub amount: Coin,
    pub receivable: Coin,
    pub amount_paid: Coin,
    pub service_type: ServiceType,
    pub status: Status,
}

pub const INVOICE: Map<&u64, Invoice> = Map::new("invoice");

pub const CONFIG: Item<Config> = Item::new("config");

pub const INVOICE_ID: Item<u64> = Item::new("invoice_id");



pub fn get_invoice_id(deps: Deps) -> u64 {
    let mut id = INVOICE_ID.load(deps.storage).unwrap_or_default();
    id += 1;
    id
}

pub fn set_config(deps: DepsMut, nft_address: Addr, owner: Addr) -> Result<Response, ContractError>{
    let config = Config {
        nft_address: nft_address,
        owner: owner,
    };
    CONFIG.save(deps.storage, &config).unwrap();
    Ok(Response::new())
}

