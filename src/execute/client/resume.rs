use crate::{
  error::{ContractError, ContractResult},
  state::CLIENTS,
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn resume(
  deps: DepsMut,
  _env: Env,
  _info: MessageInfo,
  client_address: Addr,
) -> ContractResult<Response> {
  let action = "resume";

  CLIENTS.update(
    deps.storage,
    client_address.clone(),
    |maybe_client| -> ContractResult<_> {
      if let Some(mut client) = maybe_client {
        client.is_suspended = false;
        Ok(client)
      } else {
        // client not found
        Err(ContractError::NotAuthorized {})
      }
    },
  )?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
