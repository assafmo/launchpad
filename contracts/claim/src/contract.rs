#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{Addr, DepsMut, Env, MessageInfo};
use cw2::set_contract_version;
use minter::msg::{MintCountResponse, QueryMsg};
use sg_std::{create_claim_for_msg, ClaimAction, StargazeMsgWrapper};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, SaleFinalizedHookMsg};
use crate::state::{Config, MarketplaceContract, CONFIG};
pub type Response = cosmwasm_std::Response<StargazeMsgWrapper>;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-claim";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let marketplace =
        MarketplaceContract(deps.api.addr_validate(&msg.marketplace_addr).map_err(|_| {
            ContractError::InvalidMarketplace {
                addr: msg.marketplace_addr.clone(),
            }
        })?);

    let cfg = Config { marketplace };
    CONFIG.save(deps.storage, &cfg)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ClaimMintNFT { minter_address } => {
            execute_claim_mint_nft(deps, info.sender, minter_address)
        }
        ExecuteMsg::SaleFinalizedHook(SaleFinalizedHookMsg {
            collection,
            token_id,
            seller,
            buyer,
        }) => execute_claim_buy_nft(deps, info, collection, token_id, seller, buyer),
    }
}

pub fn execute_claim_mint_nft(
    deps: DepsMut,
    sender: Addr,
    minter: String,
) -> Result<Response, ContractError> {
    let minter_addr = deps.api.addr_validate(&minter)?;
    let count_response: MintCountResponse = deps.querier.query_wasm_smart(
        minter_addr,
        &QueryMsg::MintCount {
            address: sender.to_string(),
        },
    )?;
    if count_response.count == 0 {
        return Err(ContractError::NoMinting {});
    }

    let msg = create_claim_for_msg(sender.to_string(), ClaimAction::MintNFT);
    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "claim_mint_nft")
        .add_attribute("sender", sender.to_string())
        .add_attribute("minter", minter))
}

pub fn execute_claim_buy_nft(
    deps: DepsMut,
    info: MessageInfo,
    collection: String,
    token_id: u32,
    seller: String,
    buyer: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.marketplace.addr() {
        return Err(ContractError::Unauthorized {});
    }

    let buyer = deps.api.addr_validate(&buyer)?;
    let msg = create_claim_for_msg(buyer.to_string(), ClaimAction::BidNFT);

    let res = Response::new()
        .add_message(msg)
        .add_attribute("action", "claim_buy_nft")
        .add_attribute("collection", collection)
        .add_attribute("token_id", token_id.to_string())
        .add_attribute("seller", seller)
        .add_attribute("buyer", buyer);
    Ok(res)
}

// TODO: add tests for hook after open sourcing Marketplace
// https://github.com/public-awesome/stargaze-contracts/issues/241
