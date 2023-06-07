use crate::{
  error::{ContractError, ContractResult},
  models::LiquidityUsage,
  state::{ensure_sender_is_allowed, CLIENTS, LIQUIDITY_USAGE, POOL},
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

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

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
  LIQUIDITY_USAGE.save(
    deps.storage,
    client_address.clone(),
    &LiquidityUsage {
      total_amount: Uint128::zero(),
      initial_liquidity: pool.liquidity,
      time: env.block.time,
      height: env.block.height.into(),
    },
  )?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
