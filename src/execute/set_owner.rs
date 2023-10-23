use crate::{
  error::ContractResult,
  state::{ensure_sender_is_allowed, OWNER},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};
use cw_lib::models::Owner;

pub fn set_owner(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  owner: Owner,
) -> ContractResult<Response> {
  let action = "set_owner";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "/house/set-owner")?;

  OWNER.save(deps.storage, &owner)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
