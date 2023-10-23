use crate::{
  error::ContractResult,
  state::{ensure_sender_is_allowed, suspend_client},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn suspend(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  client_address: Addr,
) -> ContractResult<Response> {
  let action = "suspend";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "/house/clients/suspend")?;
  suspend_client(deps.storage, &client_address)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
