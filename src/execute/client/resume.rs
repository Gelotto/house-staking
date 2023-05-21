use crate::error::{ContractError, ContractResult};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn resume(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  client_address: Addr,
) -> ContractResult<Response> {
  let action = "resume";
  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
