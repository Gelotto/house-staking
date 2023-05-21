use crate::error::{ContractError, ContractResult};
use crate::models::{Account, Client, Config, LedgerEntry, LedgerUpdates, Pool, TaxRecipient};
use crate::msg::InstantiateMsg;
use cosmwasm_std::{Addr, DepsMut, Env, MessageInfo, Storage};
use cw_lib::models::Owner;
use cw_storage_plus::{Item, Map};

pub const OWNER: Item<Owner> = Item::new("owner");
pub const POOL: Item<Pool> = Item::new("pool");
pub const CONFIG: Item<Config> = Item::new("config");
pub const ACCOUNTS: Map<Addr, Account> = Map::new("accounts");
pub const CLIENTS: Map<Addr, Client> = Map::new("clients");
pub const LEDGER: Map<u32, LedgerEntry> = Map::new("ledger");
pub const N_ACCOUNTS: Item<u32> = Item::new("n_accounts");
pub const N_LEDGER_ENTRIES: Item<u32> = Item::new("n_ledger_entries");
pub const N_CLIENTS: Item<u32> = Item::new("n_clients");
pub const TAX_RECIPIENTS: Map<Addr, TaxRecipient> = Map::new("tax_recipients");
pub const TAX_RATE: Item<u16> = Item::new("tax_rate");

pub fn initialize(
  deps: DepsMut,
  _env: &Env,
  info: &MessageInfo,
  msg: &InstantiateMsg,
) -> ContractResult<()> {
  OWNER.save(
    deps.storage,
    &msg
      .owner
      .clone()
      .unwrap_or_else(|| Owner::Address(info.sender.clone())),
  )?;
  POOL.save(deps.storage, &Pool::new(&msg.token))?;
  CONFIG.save(deps.storage, &msg.config)?;
  TAX_RATE.save(deps.storage, &0)?;
  N_ACCOUNTS.save(deps.storage, &0)?;
  N_CLIENTS.save(deps.storage, &0)?;
  N_LEDGER_ENTRIES.save(deps.storage, &0)?;
  Ok(())
}

pub fn sync_account(
  storage: &mut dyn Storage,
  account: &mut Account,
) -> ContractResult<()> {
  let updates = sync_account_readonly(storage, account)?;

  for (i_entry, entry) in updates.updated_entries.iter() {
    LEDGER.save(storage, *i_entry, entry)?;
  }

  for i_entry in updates.zombie_entry_indices.iter() {
    LEDGER.remove(storage, *i_entry);
  }

  Ok(())
}

pub fn sync_account_readonly(
  storage: &dyn Storage,
  account: &mut Account,
) -> ContractResult<LedgerUpdates> {
  let n_snapshots = N_LEDGER_ENTRIES.load(storage)?;
  let mut updates = LedgerUpdates {
    zombie_entry_indices: vec![],
    updated_entries: vec![],
  };

  if n_snapshots > account.offset {
    for i_entry in account.offset..n_snapshots {
      let mut entry = LEDGER.load(storage, i_entry)?;

      let gain = entry
        .delta_revenue
        .multiply_ratio(account.liquidity, entry.liquidity);
      let loss = entry
        .delta_loss
        .multiply_ratio(account.liquidity, entry.liquidity);
      let dividends = entry
        .delta_dividends
        .multiply_ratio(account.liquidity, entry.liquidity);

      entry.ref_count -= 1;

      if entry.ref_count > 0 {
        updates.updated_entries.push((i_entry, entry));
      } else {
        updates.zombie_entry_indices.push(i_entry);
      }

      account.liquidity += gain;
      account.liquidity -= loss;
      account.dividends += dividends;
    }
    account.offset = n_snapshots;
  }
  Ok(updates)
}

pub fn ensure_sender_is_valid_client(
  storage: &dyn Storage,
  addr: &Addr,
) -> ContractResult<()> {
  if let Some(client) = CLIENTS.may_load(storage, addr.clone())? {
    if client.is_suspended {
      return Err(ContractError::NotAuthorized {});
    }
  } else {
    return Err(ContractError::NotAuthorized {});
  }
  Ok(())
}

pub fn ensure_min_amount<T>(
  amount: T,
  min_amount: T,
) -> ContractResult<()>
where
  T: std::cmp::PartialOrd,
{
  if amount < min_amount {
    Err(ContractError::NotAuthorized {})
  } else {
    Ok(())
  }
}
