use crate::{
  error::ContractResult,
  models::TaxRecipient,
  state::{ensure_sender_is_allowed, insert_tax_recipients, TAX_RECIPIENTS},
};
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response};

pub fn set_taxes(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  recipients: Vec<TaxRecipient>,
) -> ContractResult<Response> {
  let action = "/house/set-taxes";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

  TAX_RECIPIENTS.clear(deps.storage);

  insert_tax_recipients(deps.storage, &recipients)?;

  Ok(Response::new().add_attribute("action", action))
}
