use std::collections::HashSet;

use cosmwasm_std::{Addr, DepsMut, Order, Response, Uint128};

use crate::{
  error::ContractResult,
  models::StakeAccount,
  state::{LEDGER, LEDGER_ENTRY_SEQ_NO, MEMOIZATION_QUEUE, N_LEDGER_ENTRIES, POOL, STAKE_ACCOUNTS},
};

/// This migration clears and resets the ledger and assigns new liquidity and
/// dividends to each account based solely on its net delegation at migration
/// time. Originally written to fix a bug whereby the latest ledger entry was
/// being erroneously and prematurely deleted by the amortization process.
/// Post-migration, the latest ledger entry may only be deleted as a result of
/// all existing stake accounts either claiming dividends or fully unstaking.
pub fn migrate(deps: DepsMut) -> ContractResult<Response> {
  // Reset ledger state.
  N_LEDGER_ENTRIES.save(deps.storage, &0)?;
  LEDGER_ENTRY_SEQ_NO.save(deps.storage, &Uint128::zero())?;
  LEDGER.clear(deps.storage);

  let pool = POOL.load(deps.storage)?;
  let mut account_addrs: HashSet<Addr> = HashSet::with_capacity(50);
  let accounts: Vec<(Addr, StakeAccount)> = STAKE_ACCOUNTS
    .range(deps.storage, None, None, Order::Ascending)
    .map(|r| {
      let (addr, account) = r.unwrap();
      account_addrs.insert(addr.clone());
      (addr, account)
    })
    .collect();

  // Redistribute liquidity, dividends & reset seq_no for each account.
  for (addr, mut account) in accounts {
    if let Some(unbonding) = &account.unbonding {
      account.delegation = unbonding.amount;
      account.unbonding = None;
    }
    account.seq_no = Uint128::zero();
    account.liquidity = pool
      .liquidity
      .multiply_ratio(account.delegation, pool.delegation);
    account.dividends = pool
      .dividends
      .multiply_ratio(account.delegation, pool.delegation);

    STAKE_ACCOUNTS.save(deps.storage, addr, &account)?;
  }

  // Get addrs in memoization queue and re-enqueue any that are missing.
  let mut enqueued_addrs: HashSet<Addr> = HashSet::with_capacity(50);
  MEMOIZATION_QUEUE.iter(deps.storage)?.for_each(|result| {
    enqueued_addrs.insert(result.unwrap().clone());
  });
  for addr in account_addrs.iter() {
    if !enqueued_addrs.contains(addr) {
      MEMOIZATION_QUEUE.push_back(deps.storage, &addr)?;
    }
  }

  Ok(Response::default())
}
