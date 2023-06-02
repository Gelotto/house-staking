use crate::{
  error::{ContractError, ContractResult},
  state::{amortize, BANK_ACCOUNTS},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn withdraw(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  amount: Option<Uint128>,
) -> ContractResult<Response> {
  let action = "withdraw";

  BANK_ACCOUNTS.update(
    deps.storage,
    info.sender.clone(),
    |maybe_account| -> ContractResult<_> {
      if let Some(mut account) = maybe_account {
        let amount = amount.unwrap_or(account.balance);
        if account.balance < amount {
          // invalid amount
          return Err(ContractError::NotAuthorized {});
        }
        account.balance -= amount;
        Ok(account)
      } else {
        return Err(ContractError::NotAuthorized {});
      }
    },
  )?;

  amortize(deps.storage)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
