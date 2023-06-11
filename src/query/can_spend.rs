use cosmwasm_std::{Addr, Deps, Env, Uint128};

use crate::{
  error::ContractResult,
  msg::CanSpendResponse,
  state::{is_rate_limited, CONFIG},
  utils::require_valid_address,
};

pub fn can_spend(
  deps: Deps,
  env: Env,
  client_address: Addr,
  spender: Addr,
  amount: Option<Uint128>,
) -> ContractResult<CanSpendResponse> {
  require_valid_address(deps.api, &client_address)?;
  require_valid_address(deps.api, &spender)?;

  let config = CONFIG.load(deps.storage)?;
  let amount = amount.unwrap_or_default();

  let is_client_rate_limited = is_rate_limited(
    deps.storage,
    &env.block,
    &config.account_rate_limit,
    &client_address,
    Some(amount),
  )?;

  let is_account_rate_limited = if spender != client_address {
    is_rate_limited(
      deps.storage,
      &env.block,
      &config.account_rate_limit,
      &spender,
      Some(amount),
    )?
  } else {
    false
  };

  Ok(CanSpendResponse {
    can_spend: !(is_account_rate_limited || is_client_rate_limited),
  })
}
