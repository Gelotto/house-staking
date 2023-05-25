use crate::{
  msg::{AccountView, Metadata, SelectResponse},
  state::{
    sync_account_readonly, BANK_ACCOUNTS, CLIENTS, CONFIG, N_CLIENTS, N_LEDGER_ENTRIES,
    N_STAKE_ACCOUNTS, OWNER, POOL, STAKE_ACCOUNTS, TAX_RECIPIENTS,
  },
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

    // house configuration settings
    config: loader.get("config", &CONFIG)?,

    // aggregate totals
    pool: loader.get("pool", &POOL)?,

    // stats and metadata about the contract
    metadata: loader.view("metadata", || {
      Ok(Some(Metadata {
        n_accounts: N_STAKE_ACCOUNTS.load(deps.storage)?,
        n_clients: N_CLIENTS.load(deps.storage)?,
        n_ledger_entries: N_LEDGER_ENTRIES.load(deps.storage)?,
      }))
    })?,

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

    // client contracts connected to the house
    clients: loader.view("clients", || {
      Ok(Some(
        CLIENTS
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| {
            let (k, mut v) = r.unwrap();
            v.address = Some(k);
            v
          })
          .collect(),
      ))
    })?,

    // sender's delegation account
    account: loader.view_by_wallet("account", wallet, |wallet| {
      let maybe_bank_account = BANK_ACCOUNTS.may_load(deps.storage, wallet.clone())?;
      let mut maybe_stake_account = STAKE_ACCOUNTS.may_load(deps.storage, wallet.clone())?;
      maybe_stake_account = if let Some(mut stake_account) = maybe_stake_account {
        if sync_account_readonly(deps.storage, &mut stake_account).is_ok() {
          Some(stake_account)
        } else {
          None
        }
      } else {
        None
      };
      Ok(Some(AccountView {
        bank: maybe_bank_account,
        stake: maybe_stake_account,
      }))
    })?,
  })
}
