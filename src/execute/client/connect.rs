use crate::{
  error::{ContractError, ContractResult},
  models::Client,
  state::{CLIENTS, N_CLIENTS},
  utils::increment,
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response};

pub fn connect(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  client: Client,
) -> ContractResult<Response> {
  let action = "connect";

  if let Some(addr) = &client.address {
    // save the client or error if already exists
    CLIENTS.update(
      deps.storage,
      addr.clone(),
      |maybe_client| -> ContractResult<_> {
        if maybe_client.is_some() {
          // already connected
          Err(ContractError::NotAuthorized {})
        } else {
          Ok(client)
        }
      },
    )?;
    // increment the client counter
    increment(deps.storage, &N_CLIENTS, 1)?;
  } else {
    // missing address
    return Err(ContractError::NotAuthorized {});
  }

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
