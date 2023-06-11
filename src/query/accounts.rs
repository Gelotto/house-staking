use std::marker::PhantomData;

use cosmwasm_std::{Addr, Deps, Order};
use cw_storage_plus::Bound;

use crate::{
  error::ContractResult,
  models::StakeAccount,
  state::{sync_account_readonly, STAKE_ACCOUNTS},
};

pub fn accounts(
  deps: Deps,
  maybe_cursor: Option<Addr>,
  maybe_limit: Option<u8>,
) -> ContractResult<Vec<StakeAccount>> {
  let limit = maybe_limit.unwrap_or(20u8) as usize;

  let range_min = maybe_cursor
    .and_then(|addr| Some(Bound::Exclusive((addr.clone(), PhantomData))))
    .or(None);

  let accounts = STAKE_ACCOUNTS
    .range(deps.storage, range_min, None, Order::Ascending)
    .take(limit)
    .map(|result| {
      let (addr, mut account) = result.unwrap();
      account.address = Some(addr);
      sync_account_readonly(deps.storage, deps.api, &mut account, true).unwrap();
      account
    })
    .collect();

  Ok(accounts)
}
