use crate::{
  error::ContractResult,
  state::{CLIENTS, N_CLIENTS},
  utils::decrement,
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn disconnect(
  deps: DepsMut,
  _env: Env,
  _info: MessageInfo,
  client_address: Addr,
) -> ContractResult<Response> {
  let action = "disconnect";

  if CLIENTS.has(deps.storage, client_address.clone()) {
    CLIENTS.remove(deps.storage, client_address.clone());
    decrement(deps.storage, &N_CLIENTS, 1)?;
  }

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
