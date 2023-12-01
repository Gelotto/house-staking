use crate::{
  error::{ContractError, ContractResult},
  models::StakeAccount,
  state::{
    ensure_has_funds, sync_account, LEDGER_ENTRY_SEQ_NO, MEMOIZATION_QUEUE, N_DELEGATION_MUTATIONS,
    N_STAKE_ACCOUNTS, POOL, STAKE_ACCOUNTS,
  },
  utils::increment,
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response, Uint128};
use cw_lib::{models::Token, utils::funds::build_cw20_transfer_from_submsg};

pub fn stake(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  amount: Uint128,
  maybe_cw20_sender: Option<Addr>,
) -> ContractResult<Response> {
  let action = "stake";
  let sender = maybe_cw20_sender.unwrap_or_else(|| info.sender.clone());
  let seq_no = LEDGER_ENTRY_SEQ_NO.load(deps.storage)?;
  let pool = POOL.load(deps.storage)?;

  // get or create the StakeAccount
  let mut account = STAKE_ACCOUNTS
    .may_load(deps.storage, sender.clone())?
    .unwrap_or_else(|| StakeAccount::new(Uint128::zero(), seq_no));

  // user must first withdraw unbonded amount before staking again
  if account.unbonding.is_some() {
    return Err(ContractError::Unbonding);
  }

  // if this is a new account, increment the global stake account counter and
  // add the staker's address to the memoization queue.
  if account.delegation.is_zero() {
    increment(deps.storage, &N_STAKE_ACCOUNTS, 1)?;
    MEMOIZATION_QUEUE.push_back(deps.storage, &sender)?;
  }

  // increment the pool's net delegation and liquidity
  POOL.update(deps.storage, |mut pool| -> ContractResult<_> {
    pool.delegation += amount;
    pool.liquidity += amount;
    Ok(pool)
  })?;

  sync_account(deps.storage, deps.api, &mut account, true)?;

  account.delegation += amount;
  account.liquidity += amount;

  STAKE_ACCOUNTS.save(deps.storage, sender.clone(), &account)?;

  // increment the delegation mutation counter, which lets the process method
  // know that a new LedgerEntry should be created when nexted executed, instead
  // of updating the existing latest entry.
  increment(deps.storage, &N_DELEGATION_MUTATIONS, Uint128::one())?;

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
        &sender,
        &env.contract.address,
        cw20_token_address,
        amount,
      )?);
    },
  }

  Ok(resp)
}
