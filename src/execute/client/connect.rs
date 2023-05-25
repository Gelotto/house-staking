use crate::{
  error::{ContractError, ContractResult},
  models::Client,
  msg::ClientInitArgs,
  state::{CLIENTS, N_CLIENTS},
  utils::increment,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn connect(
  deps: DepsMut,
  env: Env,
  _info: MessageInfo,
  init_args: ClientInitArgs,
) -> ContractResult<Response> {
  let action = "connect";

  if let Some(addr) = &init_args.address {
    // save the client or error if already exists
    CLIENTS.update(
      deps.storage,
      addr.clone(),
      |maybe_client| -> ContractResult<_> {
        if maybe_client.is_some() {
          // already connected
          Err(ContractError::NotAuthorized {})
        } else {
          Ok(Client::new(
            env.block.time,
            None,
            init_args.allowance,
            init_args.name,
            init_args.description,
            init_args.url,
          ))
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
