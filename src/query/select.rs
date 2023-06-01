use crate::{
  msg::{AccountView, Metadata, SelectResponse},
  state::{
    sync_account_readonly, BANK_ACCOUNTS, CLIENTS, CONFIG, LIQUIDITY_USAGE, N_CLIENTS,
    N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, OWNER, POOL, STAKE_ACCOUNTS, TAX_RECIPIENTS,
  },
  utils::mul_pct,
};
use cosmwasm_std::{Addr, Deps, Env, Order, StdResult, Uint128};
use cw_repository::client::Repository;

pub fn select(
  deps: Deps,
  env: Env,
  fields: Option<Vec<String>>,
  wallet: Option<Addr>,
) -> StdResult<SelectResponse> {
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
      }))
    })?,

    // tax recipients list
    taxes: loader.view("taxes", |_| {
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

      let is_suspended =
        if let Some(usage) = LIQUIDITY_USAGE.may_load(deps.storage, wallet.clone())? {
          let delta_t = env.block.time.seconds() - usage.time.seconds();
          let limit_t = config.account_rate_limit.interval_seconds.u64();
          deps
            .api
            .debug(format!(">>> delta_t: {:?}", delta_t).as_str());
          deps
            .api
            .debug(format!(">>> limit_t: {:?}", limit_t).as_str());
          if delta_t >= limit_t {
            false
          } else {
            let rate_limiting_thresh_amount = mul_pct(
              usage.initial_liquidity,
              Uint128::from(config.account_rate_limit.max_pct_change),
            );
            usage.agg_payout >= rate_limiting_thresh_amount
          }
        } else {
          false
        } && maybe_stake_account.is_some();

      maybe_stake_account = if let Some(mut stake_account) = maybe_stake_account {
        stake_account.is_suspended = Some(is_suspended);
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
        client: CLIENTS.may_load(deps.storage, wallet.clone())?,
      }))
    })?,
  })
}
