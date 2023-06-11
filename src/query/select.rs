use crate::{
  error::ContractResult,
  msg::{AccountView, LedgerEntryView, Metadata, SelectResponse},
  state::{
    is_rate_limited, sync_account_readonly, BANK_ACCOUNTS, CLIENTS, CONFIG, EVENTS, LEDGER,
    LEDGER_ENTRY_SEQ_NO, N_CLIENTS, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, OWNER, POOL,
    STAKE_ACCOUNTS, TAX_RECIPIENTS,
  },
};
use cosmwasm_std::{Addr, Deps, Env, Order};
use cw_repository::client::Repository;

pub fn select(
  deps: Deps,
  env: Env,
  fields: Option<Vec<String>>,
  wallet: Option<Addr>,
) -> ContractResult<SelectResponse> {
  let loader = Repository::loader(deps.storage, &fields, &wallet);
  let config = CONFIG.load(deps.storage)?;

  Ok(SelectResponse {
    owner: loader.get("owner", &OWNER)?,

    // house configuration settings
    config: loader.view("config", |_| Ok(Some(config.clone())))?,

    // aggregate totals
    pool: loader.get("pool", &POOL)?,

    // stats and metadata about the contract
    metadata: loader.view("metadata", |_| {
      Ok(Some(Metadata {
        n_accounts: N_STAKE_ACCOUNTS.load(deps.storage)?,
        n_clients: N_CLIENTS.load(deps.storage)?,
        n_ledger_entries: N_LEDGER_ENTRIES.load(deps.storage)?,
        ledger_entry_seq_no: LEDGER_ENTRY_SEQ_NO.load(deps.storage)?,
      }))
    })?,

    events: loader.view("events", |_| {
      Ok(Some(
        EVENTS
          .iter(deps.storage)?
          .take(20)
          .map(|x| x.unwrap())
          .collect(),
      ))
    })?,

    ledger: loader.view("ledger", |_| {
      Ok(Some(
        LEDGER
          .range(deps.storage, None, None, Order::Ascending)
          .take(20)
          .map(|x| {
            let (k, v) = x.unwrap();
            LedgerEntryView {
              seq_no: k.into(),
              entry: v,
            }
          })
          .collect(),
      ))
    })?,

    // tax recipients list
    taxes: loader.view("taxes", |_| {
      Ok(Some(
        TAX_RECIPIENTS
          .range(deps.storage, None, None, Order::Ascending)
          .map(|r| {
            let (addr, mut recipient) = r.unwrap();
            recipient.address = Some(addr);
            recipient
          })
          .collect(),
      ))
    })?,

    // client contracts connected to the house
    clients: loader.view("clients", |_| {
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
    account: loader.view("account", |maybe_wallet| {
      if maybe_wallet.is_none() {
        return Ok(None);
      }

      let wallet = maybe_wallet.unwrap();
      let maybe_bank_account = BANK_ACCOUNTS.may_load(deps.storage, wallet.clone())?;
      let mut maybe_stake_account = STAKE_ACCOUNTS.may_load(deps.storage, wallet.clone())?;
      let is_suspended = is_rate_limited(
        deps.storage,
        &env.block,
        &config.account_rate_limit,
        &wallet,
        None,
      )
      .unwrap_or(false);

      maybe_stake_account = if let Some(mut stake_account) = maybe_stake_account {
        sync_account_readonly(deps.storage, deps.api, &mut stake_account, true).unwrap();
        Some(stake_account)
      } else {
        None
      };

      Ok(Some(AccountView {
        bank: maybe_bank_account,
        stake: maybe_stake_account,
        client: CLIENTS.may_load(deps.storage, wallet.clone())?,
        is_suspended,
      }))
    })?,
  })
}
