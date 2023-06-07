use crate::{
  error::{ContractError, ContractResult},
  models::ClientConfig,
  state::{ensure_sender_is_allowed, CLIENTS},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn set_client_config(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  client_address: Addr,
  config: ClientConfig,
) -> ContractResult<Response> {
  let action = "set_client_config";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

  CLIENTS.update(
    deps.storage,
    client_address.clone(),
    |maybe_client| -> ContractResult<_> {
      if let Some(mut client) = maybe_client {
        client.config = config;
        Ok(client)
      } else {
        Err(ContractError::ClientNotFound)
      }
    },
  )?;

  Ok(Response::new().add_attributes(vec![
    attr("action", action),
    attr("client_address", client_address.to_string()),
  ]))
}
