use crate::{
  error::{ContractError, ContractResult},
  state::BANK_ACCOUNTS,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn deposit(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  amount: Uint128,
) -> ContractResult<Response> {
  let action = "deposit";

  if amount.is_zero() {
    // invalid amount
    return Err(ContractError::NotAuthorized {});
  }

  BANK_ACCOUNTS.update(
    deps.storage,
    info.sender.clone(),
    |maybe_account| -> ContractResult<_> {
      if let Some(mut account) = maybe_account {
        account.balance += amount;
        Ok(account)
      } else {
        return Err(ContractError::NotAuthorized {});
      }
    },
  )?;

  // TODO: build transfer msg

  Ok(Response::new().add_attributes(vec![
    attr("action", action),
    attr("amount", amount.to_string()),
  ]))
}
