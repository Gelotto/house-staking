use crate::{
  error::ContractResult,
  models::Config,
  state::{ensure_sender_is_allowed, CONFIG},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};

pub fn set_config(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  config: Config,
) -> ContractResult<Response> {
  let action = "set_config";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

  CONFIG.save(deps.storage, &config)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
