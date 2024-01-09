use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Contact {
    pub alias : String ,
    pub contact_address : Addr ,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ContactInfo {
    pub sent_requests : Vec<Contact> ,
    pub received_requests : Vec<Contact> ,
    pub contacts : Vec<Contact> ,
}

pub const CONTACT_INFO: Map<&Addr, ContactInfo> = Map::new("contact_info");
