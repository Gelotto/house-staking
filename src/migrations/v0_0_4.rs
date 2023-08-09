use crate::{
  error::ContractResult,
  state::{LEDGER, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS_UNBONDING, STAKE_ACCOUNTS},
  utils::decrement,
};
use cosmwasm_std::{DepsMut, Order, Response, Uint128};

/// Initialize N_STAKE_ACCOUNTS_UNBONDING. Clean up stale ledger entries that
/// stuck around due to the fact that unbonding accounts were included in the
/// entries' ref_counts.
pub fn migrate(deps: DepsMut) -> ContractResult<Response> {
  let mut min_seq_no = Uint128::MAX;
  let mut n_unbonding: u32 = 0;

  STAKE_ACCOUNTS
    .range(deps.storage, None, None, Order::Ascending)
    .for_each(|r| {
      let account = r.unwrap().1;
      if account.unbonding.is_some() {
        n_unbonding += 1;
      }
      if account.seq_no < min_seq_no {
        min_seq_no = account.seq_no;
      }
    });

  // init unbonding account count
  N_STAKE_ACCOUNTS_UNBONDING.save(deps.storage, &n_unbonding)?;

  // collect stale ledger entries
  let mut stale_seq_nos: Vec<u128> = Vec::with_capacity(16);
  for seq_no in LEDGER
    .keys(deps.storage, None, None, Order::Ascending)
    .map(|r| r.unwrap())
  {
    if seq_no < min_seq_no.u128() {
      stale_seq_nos.push(seq_no)
    }
  }

  // remove stale ledger entries
  for seq_no in stale_seq_nos.iter() {
    LEDGER.remove(deps.storage, *seq_no);
  }

  // reduce net ledger entry count
  decrement(deps.storage, &N_LEDGER_ENTRIES, stale_seq_nos.len() as u32)?;

  Ok(Response::default())
}
