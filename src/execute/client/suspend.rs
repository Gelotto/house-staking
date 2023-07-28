use crate::{
  error::{ContractError, ContractResult},
  state::{ensure_sender_is_allowed, CLIENTS},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn suspend(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  client_address: Addr,
) -> ContractResult<Response> {
  let action = "suspend";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

  CLIENTS.update(
    deps.storage,
    client_address.clone(),
    |maybe_client| -> ContractResult<_> {
      if let Some(mut client) = maybe_client {
        client.is_suspended = true;
        Ok(client)
      } else {
        // client not found
        Err(ContractError::NotAuthorized {})
      }
    },
  )?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
