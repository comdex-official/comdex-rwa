use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::query::ApprovalResponse;
use crate::{
    AllNftInfoResponse, ApprovalsResponse, ContractInfoResponse, NftInfoResponse,
    NumTokensResponse, OperatorsResponse, OwnerOfResponse, TokensResponse,
};
use cosmwasm_std::{Binary, CustomMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw_utils::Expiration;

pub trait Cw721<MintExt, ResponseExt, CollectionMetadataExt>:
    Cw721Execute<ResponseExt> + Cw721Query<MintExt, ResponseExt>
where
    MintExt: Serialize + DeserializeOwned + Clone,
    ResponseExt: CustomMsg,
{
}

pub trait Cw721Execute<ResponseExt>
where
    ResponseExt: CustomMsg,
{
    type Err: ToString;

    fn transfer_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        recipient: String,
        token_id: String,
    ) -> Result<Response<ResponseExt>, Self::Err>;

    fn send_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        contract: String,
        token_id: String,
        msg: Binary,
    ) -> Result<Response<ResponseExt>, Self::Err>;

    fn approve(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        spender: String,
        token_id: String,
        expires: Option<Expiration>,
    ) -> Result<Response<ResponseExt>, Self::Err>;

    fn revoke(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        spender: String,
        token_id: String,
    ) -> Result<Response<ResponseExt>, Self::Err>;

    fn approve_all(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        operator: String,
        expires: Option<Expiration>,
    ) -> Result<Response<ResponseExt>, Self::Err>;

    fn revoke_all(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        operator: String,
    ) -> Result<Response<ResponseExt>, Self::Err>;

    fn burn(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        token_id: String,
    ) -> Result<Response<ResponseExt>, Self::Err>;
}

pub trait Cw721Query<MintExt, CollectionMetadataExt>
where
    MintExt: Serialize + DeserializeOwned + Clone,
    CollectionMetadataExt: CustomMsg,
{
    fn contract_info(&self, deps: Deps) -> StdResult<ContractInfoResponse<CollectionMetadataExt>>;

    fn num_tokens(&self, deps: Deps) -> StdResult<NumTokensResponse>;

    fn nft_info(&self, deps: Deps, token_id: String) -> StdResult<NftInfoResponse<MintExt>>;

    fn owner_of(
        &self,
        deps: Deps,
        env: Env,
        token_id: String,
        include_expired: bool,
    ) -> StdResult<OwnerOfResponse>;

    fn operators(
        &self,
        deps: Deps,
        env: Env,
        owner: String,
        include_expired: bool,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<OperatorsResponse>;

    fn approval(
        &self,
        deps: Deps,
        env: Env,
        token_id: String,
        spender: String,
        include_expired: bool,
    ) -> StdResult<ApprovalResponse>;

    fn approvals(
        &self,
        deps: Deps,
        env: Env,
        token_id: String,
        include_expired: bool,
    ) -> StdResult<ApprovalsResponse>;

    fn tokens(
        &self,
        deps: Deps,
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<TokensResponse>;

    fn all_tokens(
        &self,
        deps: Deps,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<TokensResponse>;

    fn all_nft_info(
        &self,
        deps: Deps,
        env: Env,
        token_id: String,
        include_expired: bool,
    ) -> StdResult<AllNftInfoResponse<MintExt>>;
}
