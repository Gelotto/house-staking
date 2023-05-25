use crate::{
  error::{ContractError, ContractResult},
  models::StakeAccount,
  state::{
    ensure_has_funds, sync_account, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, POOL, STAKE_ACCOUNTS,
  },
  utils::increment,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};
use cw_lib::{models::Token, utils::funds::build_cw20_transfer_from_submsg};

pub fn stake(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  amount: Uint128,
) -> ContractResult<Response> {
  let action = "stake";
  let n_entry = N_LEDGER_ENTRIES.load(deps.storage)?;
  let pool = POOL.load(deps.storage)?;

  // get or create the StakeAccount
  let mut account = STAKE_ACCOUNTS
    .may_load(deps.storage, info.sender.clone())?
    .unwrap_or_else(|| StakeAccount::new(Uint128::zero(), n_entry));

  // user must first withdraw unbonded amount before staking again
  if account.unbonding.is_some() {
    return Err(ContractError::NotAuthorized {});
  }

  // increment account counter if this is a new account
  if account.delegation.is_zero() {
    increment(deps.storage, &N_STAKE_ACCOUNTS, 1)?;
  }

  // increment the pool's net delegation and liquidity
  POOL.update(deps.storage, |mut pool| -> ContractResult<_> {
    pool.delegation += amount;
    pool.liquidity += amount;
    Ok(pool)
  })?;

  sync_account(deps.storage, &mut account)?;

  account.delegation += amount;
  account.liquidity += amount;

  STAKE_ACCOUNTS.save(deps.storage, info.sender.clone(), &account)?;

  let mut resp = Response::new().add_attributes(vec![
    attr("action", action),
    attr("amount", amount.to_string()),
  ]);

  // ensure the sender has required funds and build any necessary
  // submsg to perform the transfer from sender to the house.
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

  Ok(resp)
}
