use crate::error::ContractResult;
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn withdraw(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "withdraw";
  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
