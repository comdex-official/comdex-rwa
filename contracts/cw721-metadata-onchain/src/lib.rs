#![allow(unused_imports, unused_variables, dead_code)]
mod msg;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{from_json, DepsMut, Empty, Env, MessageInfo, Response, StdError, Uint128};
use cw2::set_contract_version;
use cw721::NftInfoResponse;
pub use cw721_base::{ContractError, InstantiateMsg, MigrateMsg, MintMsg, MinterResponse};

// Version info for migration
const CONTRACT_NAME: &str = "crates.io:cw721-metadata-onchain";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cw_serde]
pub struct LendInfo {
    pub principal_deposited: Uint128,
    pub principal_redeemed: Uint128,
    pub interest_redeemed: Uint128,
}

impl Default for LendInfo {
    fn default() -> Self {
        LendInfo {
            principal_deposited: Uint128::zero(),
            principal_redeemed: Uint128::zero(),
            interest_redeemed: Uint128::zero(),
        }
    }
}

#[cw_serde]
pub struct InvestorToken {
    pub token_id: u128,
    pub pool_id: u64,
    pub tranche_id: u64,
    pub lend_info: LendInfo,
}

impl InvestorToken {
    pub fn new(token_id: u128, pool_id: u64, tranche_id: u64) -> Self {
        InvestorToken {
            token_id,
            pool_id,
            tranche_id,
            lend_info: LendInfo::default(),
        }
    }
}
pub type Extension = Option<InvestorToken>;

//pub type Cw721MetadataContract<'a> =
//cw721_base::Cw721Contract<'a, Extension, Empty, Empty, Empty, Empty>;
pub type ExecuteMsg = cw721_base::ExecuteMsg<Extension, Empty>;
pub type QueryMsg = cw721_base::QueryMsg<Empty>;

#[derive(Default)]
pub struct Cw721MetadataContract<'a> {
    base: cw721_base::Cw721Contract<'a, Extension, Empty, Empty, Empty, Empty>,
}

impl<'a> Cw721MetadataContract<'a> {
    pub fn withdraw_principal(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        token_id: String,
        principal_amount: Uint128,
    ) -> Result<Response, ContractError> {
        let minter = self.base.minter.load(deps.as_ref().storage)?;
        if info.sender != minter {
            return Err(ContractError::Unauthorized {});
        }
        let mut nft = self.base.tokens.load(deps.as_ref().storage, &token_id)?;
        let investor_token = nft.extension.as_mut().unwrap();
        if investor_token.lend_info.principal_deposited < principal_amount {
            return Err(StdError::generic_err(
                "Withdrawal amount exceeds deposited amount",
            ))?;
        }
        if investor_token.lend_info.principal_redeemed.is_zero() {
            return Err(StdError::generic_err(
                "Principal already redeemed, withdrawal not allowed",
            ))?;
        }
        investor_token.lend_info.principal_deposited = investor_token
            .lend_info
            .principal_deposited
            .checked_sub(principal_amount)
            .map_err(|_| StdError::generic_err("Withdrawal: Underflow"))?;

        self.base.tokens.save(deps.storage, &token_id, &nft)?;

        Ok(Response::new()
            .add_attribute("method", "withdraw_principal")
            .add_attribute("token_id", token_id)
            .add_attribute("amount_withdrawn", principal_amount.to_string()))
    }

    pub fn redeem(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        token_id: String,
        principal_redeemed: Uint128,
        interest_redeemed: Uint128,
    ) -> Result<Response, ContractError> {
        let minter = self.base.minter.load(deps.as_ref().storage)?;
        if info.sender != minter {
            return Err(ContractError::Unauthorized {});
        }
        let mut nft = self.base.tokens.load(deps.as_ref().storage, &token_id)?;
        let investor_token = nft.extension.as_mut().unwrap();
        investor_token.lend_info.principal_redeemed = investor_token
            .lend_info
            .principal_redeemed
            .checked_add(principal_redeemed)
            .map_err(|_| StdError::generic_err("Redeem: Overflow"))?;
        investor_token.lend_info.interest_redeemed = investor_token
            .lend_info
            .interest_redeemed
            .checked_add(interest_redeemed)
            .map_err(|_| StdError::generic_err("Redeem: Overflow"))?;

        self.base.tokens.save(deps.storage, &token_id, &nft)?;

        Ok(Response::new()
            .add_attribute("method", "redeem")
            .add_attribute("token_id", token_id)
            .add_attribute("principal_redeemed", principal_redeemed.to_string())
            .add_attribute("interest_redeemed", interest_redeemed.to_string()))
    }
}

//#[cfg(not(feature = "library"))]
pub mod entry {
    use super::*;

    use cosmwasm_std::entry_point;
    use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
    use cw721_base::msg::MigrateMsg;

    // This makes a conscious choice on the various generics used by the contract
    #[entry_point]
    pub fn instantiate(
        mut deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: InstantiateMsg<Empty>,
    ) -> Result<Response, ContractError> {
        let res =
            Cw721MetadataContract::default()
                .base
                .instantiate(deps.branch(), env, info, msg)?;
        // Explicitly set contract name and version, otherwise set to cw721-base info
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)
            .map_err(ContractError::Std)?;
        Ok(res)
    }

    #[entry_point]
    pub fn execute(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        Cw721MetadataContract::default()
            .base
            .execute(deps, env, info, msg)
    }

    #[entry_point]
    pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
        Cw721MetadataContract::default().base.query(deps, env, msg)
    }

    #[entry_point]
    pub fn migrate(
        deps: DepsMut,
        env: Env,
        msg: MigrateMsg<Empty>,
    ) -> Result<Response, ContractError> {
        Cw721MetadataContract::default()
            .base
            .migrate(deps, env, msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        to_json_binary, Addr, CosmosMsg, WasmMsg,
    };
    use cw721::Cw721Query;
    use cw_multi_test::{App, Contract, ContractWrapper, Executor};

    fn cw721_metadata_onchain_v0134_contract_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw721_metadata_onchain_v0134_contract::entry::execute,
            cw721_metadata_onchain_v0134_contract::entry::instantiate,
            cw721_metadata_onchain_v0134_contract::entry::query,
        );
        Box::new(contract)
    }

    fn cw721_base_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::entry::execute,
            crate::entry::instantiate,
            crate::entry::query,
        )
        .with_migrate(crate::entry::migrate);
        Box::new(contract)
    }

    const CREATOR: &str = "creator";
    const CONTRACT_NAME: &str = "Magic Power";
    const CONTRACT_URI: &str = "https://example.com/example.jpg";
    const SYMBOL: &str = "MGK";

    #[test]
    fn use_metadata_extension() {
        let mut deps = mock_dependencies();
        let contract = Cw721MetadataContract::default();

        let info = mock_info(CREATOR, &[]);
        let init_msg = InstantiateMsg::<Empty> {
            name: CONTRACT_NAME.to_string(),
            symbol: SYMBOL.to_string(),
            collection_uri: Some(String::from(CONTRACT_URI)),
            metadata: Empty {},
            minter: CREATOR.to_string(),
        };
        contract
            .base
            .instantiate(deps.as_mut(), mock_env(), info.clone(), init_msg)
            .unwrap();

        let token_id = "Enterprise";
        let mint_msg = MintMsg {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            token_uri: Some("https://starships.example.com/Starship/Enterprise.json".into()),
            extension: Some(Metadata {
                description: Some("Spaceship with Warp Drive".into()),
                name: Some("Starship USS Enterprise".to_string()),
                ..Metadata::default()
            }),
        };
        let exec_msg = ExecuteMsg::Mint(mint_msg.clone());
        contract
            .execute(deps.as_mut(), mock_env(), info, exec_msg)
            .unwrap();

        let res = contract.nft_info(deps.as_ref(), token_id.into()).unwrap();
        assert_eq!(res.token_uri, mint_msg.token_uri);
        assert_eq!(res.extension, mint_msg.extension);
    }

    #[test]
    fn test_migrate_from_v0134() {
        const CREATOR: &str = "creator";

        let mut app = App::default();
        let v0134_code_id = app.store_code(cw721_metadata_onchain_v0134_contract_contract());

        // Instantiate old NFT contract
        let v0134_addr = app
            .instantiate_contract(
                v0134_code_id,
                Addr::unchecked(CREATOR),
                &cw721_metadata_onchain_v0134_contract::InstantiateMsg {
                    name: "Test".to_string(),
                    symbol: "TEST".to_string(),
                    minter: CREATOR.to_string(),
                },
                &[],
                "Old cw721-base",
                Some(CREATOR.to_string()),
            )
            .unwrap();

        let cw721_base_code_id = app.store_code(cw721_base_contract());

        // Now we can migrate!
        app.execute(
            Addr::unchecked(CREATOR),
            CosmosMsg::Wasm(WasmMsg::Migrate {
                contract_addr: v0134_addr.to_string(),
                new_code_id: cw721_base_code_id,
                msg: to_json_binary(&MigrateMsg::<Empty> {
                    name: "Test".to_string(),
                    symbol: "TEST".to_string(),
                    collection_uri: Some("https://ipfs.io/hash".to_string()),
                    metadata: Empty {},
                })
                .unwrap(),
            }),
        )
        .unwrap();
    }
}
