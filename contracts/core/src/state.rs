use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;
use cosmwasm_std::{Addr, Coin, Deps, DepsMut, Response};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Asset {
    pub name: String,
    pub denom: String,
    pub decimal: u64,
    pub uri: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub nft_address: Addr,
    pub owner: Addr,
    pub accepted_assets: Vec<Asset>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Contact {
    pub alias: String,
    pub contact_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Metadata {
    pub invoice_id: u64,
    pub from: Addr,
    pub payee_address: Addr,
    pub uri: String,
    pub receivable: Coin,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ContactInfo {
    pub name: String,
    pub company_name: String,
    pub address: String,
    pub phone_number: String,
    pub owner: Addr,
    pub email_id: String,
    pub sent_requests: Vec<Addr>,
    pub received_requests: Vec<Addr>,
    pub contacts: Vec<Addr>,
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
    Raised,
    Accepted,
    Paid,
    PartiallyPaid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KYCStatus {
    Unverified,
    InProcess,
    Rejected,
    Approved,
    ReVerify,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Invoice {
    pub id: u64,
    pub from: Addr,
    pub payee_address: Addr,
    pub nft_id: u64,
    pub doc_uri: String,
    pub due_amount: Coin,
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

pub fn set_config(
    deps: DepsMut,
    nft_address: Addr,
    owner: Addr,
    accepted_assets: Vec<Asset>,
) -> Result<Response, ContractError> {
    
    let config = Config {
        nft_address: nft_address,
        owner: owner,
        accepted_assets: accepted_assets,
    };
    CONFIG.save(deps.storage, &config).unwrap();
    Ok(Response::new())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}
