use crate::{
  error::{ContractError, ContractResult},
  state::{amortize, ensure_has_funds, BANK_ACCOUNTS, POOL},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};
use cw_lib::{models::Token, utils::funds::build_cw20_transfer_from_submsg};

pub fn deposit(
  deps: DepsMut,
  env: Env,
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

  let pool = POOL.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![
    attr("action", action),
    attr("amount", amount.to_string()),
  ]);

  // validate and take payment
  match &pool.token {
    Token::Native { denom } => {
      ensure_has_funds(&info.funds, denom, amount)?;
    },
    Token::Cw20 {
      address: cw20_token_address,
    } => {
      resp = resp.add_submessage(build_cw20_transfer_from_submsg(
        &info.sender,
        &env.contract.address,
        cw20_token_address,
        amount,
      )?);
    },
  }

  amortize(deps.storage, deps.api)?;

  Ok(resp)
}
