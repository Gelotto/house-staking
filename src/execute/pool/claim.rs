use crate::error::ContractResult;
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn claim(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "claim";
  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
