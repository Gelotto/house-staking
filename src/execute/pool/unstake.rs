use crate::{
  error::{ContractError, ContractResult},
  state::{sync_account, ACCOUNTS, POOL},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn unstake(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "unstake";
  let pool = POOL.load(deps.storage)?;
  let mut account = ACCOUNTS.load(deps.storage, info.sender.clone())?;

  sync_account(deps.storage, &mut account)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
