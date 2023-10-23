use crate::{
  error::{ContractError, ContractResult},
  models::Usage,
  state::{ensure_sender_is_allowed, CLIENTS, POOL, USAGE},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn resume(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  client_address: Addr,
) -> ContractResult<Response> {
  let action = "resume";
  let pool = POOL.load(deps.storage)?;

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, "/house/clients/resume")?;

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

  // clear the client's liquidity usage
  USAGE.save(
    deps.storage,
    client_address.clone(),
    &Usage {
      spent: Uint128::zero(),
      added: Uint128::zero(),
      prev_height: env.block.height.into(),
      start_liquidity: pool.liquidity,
      start_time: env.block.time,
    },
  )?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
