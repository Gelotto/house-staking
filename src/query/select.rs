use crate::{
  msg::SelectResponse,
  state::{sync_account_readonly, ACCOUNTS, CONFIG, OWNER, POOL, TAX_RECIPIENTS},
};
use cosmwasm_std::{Addr, Deps, Order, StdResult};
use cw_repository::client::Repository;

pub fn select(
  deps: Deps,
  fields: Option<Vec<String>>,
  wallet: Option<Addr>,
) -> StdResult<SelectResponse> {
  let loader = Repository::loader(deps.storage, &fields);
  Ok(SelectResponse {
    owner: loader.get("owner", &OWNER)?,

    config: loader.get("config", &CONFIG)?,

    pool: loader.get("pool", &POOL)?,

    // tax recipients list
    taxes: loader.view("taxes", || {
      Ok(Some(
        TAX_RECIPIENTS
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| {
            let (addr, mut recipient) = r.unwrap();
            recipient.addr = Some(addr);
            recipient
          })
          .collect(),
      ))
    })?,

    // sender's delegation account
    account: loader.view_by_wallet("account", wallet, |wallet| {
      Ok(
        if let Some(mut account) = ACCOUNTS.may_load(deps.storage, wallet.to_owned())? {
          if sync_account_readonly(deps.storage, &mut account).is_ok() {
            Some(account)
          } else {
            None
          }
        } else {
          None
        },
      )
    })?,
  })
}
