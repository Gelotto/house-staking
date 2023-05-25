use crate::{
  error::ContractResult,
  models::{Client, LedgerEntry},
  state::{
    ensure_min_amount, ensure_sender_is_valid_client, BANK_ACCOUNTS, LEDGER, N_LEDGER_ENTRIES,
    N_STAKE_ACCOUNTS, POOL,
  },
  utils::increment,
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response, Uint128};
use cw_lib::utils::funds::build_send_submsg;

pub fn pay(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  payment: Uint128,
  recipient: Addr,
) -> ContractResult<Response> {
  ensure_min_amount(payment, Uint128::one())?;
  ensure_sender_is_valid_client(
    deps.storage,
    &info.sender,
    Some(&|client: &mut Client| client.expenditure += payment),
  )?;

  let action = "pay";
  let i_entry = increment(deps.storage, &N_LEDGER_ENTRIES, 1)? - 1;
  let mut pool = POOL.load(deps.storage)?;
  let entry = LedgerEntry {
    ref_count: N_STAKE_ACCOUNTS.load(deps.storage)?,
    liquidity: pool.liquidity,
    delegation: pool.delegation,
    delta_revenue: Uint128::zero(),
    delta_dividends: Uint128::zero(),
    delta_loss: payment,
  };

  // outlays are paid entirely from house liquidity
  pool.liquidity -= entry.delta_loss;

  LEDGER.save(deps.storage, i_entry, &entry)?;
  POOL.save(deps.storage, &pool)?;

  let mut resp = Response::new().add_attributes(vec![attr("action", action)]);

  // if the recipient address has a bank acocunt, simply increment its balance;
  // otherwise, send the recipient GLTO directly.
  if let Some(mut bank_account) = BANK_ACCOUNTS.may_load(deps.storage, recipient.clone())? {
    bank_account.balance += payment;
    BANK_ACCOUNTS.save(deps.storage, recipient.clone(), &bank_account)?;
  } else {
    // transfer GLTO to recipient directly
    resp = resp.add_submessage(build_send_submsg(&recipient, payment, &pool.token)?);
  }

  Ok(resp)
}
