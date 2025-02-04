#[cfg(not(feature = "library"))]
use cosmwasm_std::{DepsMut, Env, Event, MessageInfo};
use cw2::set_contract_version;
use sg721_base::msg::CollectionInfoResponse;

use crate::error::ContractError;
use crate::state::FROZEN_TOKEN_METADATA;
use sg721::InstantiateMsg;

use cw721_base::Extension;
use cw_utils::nonpayable;
use sg721_base::ContractError::Unauthorized;
use sg721_base::Sg721Contract;
pub type Sg721UpdatableContract<'a> = Sg721Contract<'a, Extension>;
use sg_std::Response;

const CONTRACT_NAME: &str = "crates.io:sg721-updatable";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn _instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Set frozen to false on instantiate. allows updating token metadata
    FROZEN_TOKEN_METADATA.save(deps.storage, &false)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let res = Sg721UpdatableContract::default().instantiate(deps, env, info, msg)?;
    Ok(res)
}

pub fn execute_freeze_token_metadata(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    // Check if sender is creator
    let collection_info: CollectionInfoResponse =
        Sg721UpdatableContract::default().query_collection_info(deps.as_ref())?;
    if info.sender != collection_info.creator {
        return Err(ContractError::Base(Unauthorized {}));
    }

    FROZEN_TOKEN_METADATA.save(deps.storage, &true)?;

    Ok(Response::new()
        .add_attribute("action", "freeze_token_metadata")
        .add_attribute("frozen", "true"))
}

pub fn execute_update_token_metadata(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token_id: String,
    token_uri: Option<String>,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    // Check if sender is creator
    let owner = deps.api.addr_validate(info.sender.as_ref())?;
    let collection_info: CollectionInfoResponse =
        Sg721UpdatableContract::default().query_collection_info(deps.as_ref())?;
    if owner != collection_info.creator {
        return Err(ContractError::Base(Unauthorized {}));
    }

    // Check if token metadata is frozen
    let frozen = FROZEN_TOKEN_METADATA.load(deps.storage)?;
    if frozen {
        return Err(ContractError::TokenMetadataFrozen {});
    }

    // Update token metadata
    Sg721UpdatableContract::default().tokens.update(
        deps.storage,
        &token_id,
        |token| match token {
            Some(mut token_info) => {
                token_info.token_uri = token_uri.clone();
                Ok(token_info)
            }
            None => Err(ContractError::TokenIdNotFound {}),
        },
    )?;

    let event = Event::new("update_update_token_metadata")
        .add_attribute("sender", info.sender)
        .add_attribute("token_id", token_id)
        .add_attribute("token_uri", token_uri.unwrap_or_default());
    Ok(Response::new().add_event(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::entry::{execute, instantiate};
    use crate::msg::ExecuteMsg;
    use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::{
        from_slice, to_binary, ContractInfoResponse, ContractResult, Empty, OwnedDeps, Querier,
        QuerierResult, QueryRequest, SystemError, SystemResult, WasmQuery,
    };
    use cw721::Cw721Query;
    use cw721_base::MintMsg;
    use sg721::{CollectionInfo, InstantiateMsg};
    use std::marker::PhantomData;

    const CREATOR: &str = "creator";
    const HACKER: &str = "hacker";

    pub fn mock_deps() -> OwnedDeps<MockStorage, MockApi, CustomMockQuerier, Empty> {
        OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: CustomMockQuerier::new(MockQuerier::new(&[])),
            custom_query_type: PhantomData,
        }
    }

    pub struct CustomMockQuerier {
        base: MockQuerier,
    }

    impl Querier for CustomMockQuerier {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            let request: QueryRequest<Empty> = match from_slice(bin_request) {
                Ok(v) => v,
                Err(e) => {
                    return SystemResult::Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })
                }
            };

            self.handle_query(&request)
        }
    }

    impl CustomMockQuerier {
        pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
            match &request {
                QueryRequest::Wasm(WasmQuery::ContractInfo { contract_addr: _ }) => {
                    let mut response = ContractInfoResponse::default();
                    response.code_id = 1;
                    response.creator = CREATOR.to_string();
                    SystemResult::Ok(ContractResult::Ok(to_binary(&response).unwrap()))
                }
                _ => self.base.handle_query(request),
            }
        }

        pub fn new(base: MockQuerier<Empty>) -> Self {
            CustomMockQuerier { base }
        }
    }

    #[test]
    fn update_token_metadata() {
        let mut deps = mock_deps();
        let contract = Sg721UpdatableContract::default();

        // Instantiate contract
        let info = mock_info(CREATOR, &[]);
        let init_msg = InstantiateMsg {
            name: "SpaceShips".to_string(),
            symbol: "SPACE".to_string(),
            minter: CREATOR.to_string(),
            collection_info: CollectionInfo {
                creator: CREATOR.to_string(),
                description: "this is a test".to_string(),
                image: "https://larry.engineer".to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            },
        };
        instantiate(deps.as_mut(), mock_env(), info.clone(), init_msg).unwrap();

        // Mint token
        let token_id = "Enterprise";
        let mint_msg = MintMsg {
            token_id: token_id.to_string(),
            owner: "john".to_string(),
            token_uri: Some("https://starships.example.com/Starship/Enterprise.json".into()),
            extension: None,
        };
        let exec_msg = ExecuteMsg::Mint(mint_msg);
        execute(deps.as_mut(), mock_env(), info.clone(), exec_msg).unwrap();

        // Update token metadata fails because token id is not found
        let updated_token_uri = Some("https://badkids.example.com/collection-cid/1.json".into());
        let update_msg = ExecuteMsg::UpdateTokenMetadata {
            token_id: "wrong-token-id".to_string(),
            token_uri: updated_token_uri.clone(),
        };
        let err = execute(deps.as_mut(), mock_env(), info.clone(), update_msg).unwrap_err();
        assert_eq!(
            err.to_string(),
            ContractError::TokenIdNotFound {}.to_string()
        );

        // Update token metadata fails because sent by hacker
        let update_msg = ExecuteMsg::UpdateTokenMetadata {
            token_id: token_id.to_string(),
            token_uri: updated_token_uri.clone(),
        };
        let hacker_info = mock_info(HACKER, &[]);
        let err = execute(deps.as_mut(), mock_env(), hacker_info, update_msg.clone()).unwrap_err();
        assert_eq!(
            err.to_string(),
            ContractError::Base(Unauthorized {}).to_string()
        );

        // Update token metadata
        execute(deps.as_mut(), mock_env(), info.clone(), update_msg).unwrap();

        // Check token contains updated metadata
        let res = contract
            .parent
            .nft_info(deps.as_ref(), token_id.into())
            .unwrap();
        assert_eq!(res.token_uri, updated_token_uri);

        // Update token metadata with None token_uri
        let update_msg = ExecuteMsg::<Extension, Empty>::UpdateTokenMetadata {
            token_id: token_id.to_string(),
            token_uri: None,
        };
        execute(deps.as_mut(), mock_env(), info.clone(), update_msg).unwrap();
        let res = contract
            .parent
            .nft_info(deps.as_ref(), token_id.into())
            .unwrap();
        assert_eq!(res.token_uri, None);

        // Freeze token metadata
        let freeze_msg = ExecuteMsg::FreezeTokenMetadata {};
        execute(deps.as_mut(), mock_env(), info.clone(), freeze_msg).unwrap();

        // Throws error trying to update token metadata
        let updated_token_uri =
            Some("https://badkids.example.com/other-collection-cid/2.json".into());
        let update_msg = ExecuteMsg::UpdateTokenMetadata {
            token_id: token_id.to_string(),
            token_uri: updated_token_uri,
        };
        let err = execute(deps.as_mut(), mock_env(), info, update_msg).unwrap_err();
        assert_eq!(
            err.to_string(),
            ContractError::TokenMetadataFrozen {}.to_string()
        );
    }
}
