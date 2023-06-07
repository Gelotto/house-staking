use crate::{
  error::{ContractError, ContractResult},
  models::Client,
  msg::ClientInitArgs,
  state::{ensure_sender_is_allowed, CLIENTS, CONFIG, N_CLIENTS},
  utils::increment,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn connect(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  init_args: ClientInitArgs,
) -> ContractResult<Response> {
  let action = "connect";
  let config = CONFIG.load(deps.storage)?;

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

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
            init_args.budget,
            init_args.name,
            init_args.description,
            init_args.url,
            init_args
              .rate_limit
              .unwrap_or(config.default_client_rate_limit.clone()),
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
