use crate::{error::ContractResult, models::Config, state::CONFIG};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn set_config(
  deps: DepsMut,
  _env: Env,
  _info: MessageInfo,
  config: Config,
) -> ContractResult<Response> {
  let action = "set_config";
  CONFIG.save(deps.storage, &config)?;
  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
