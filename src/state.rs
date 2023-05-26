use crate::error::{ContractError, ContractResult};
use crate::models::{
  BankAccount, Client, Config, LedgerEntry, LedgerUpdates, LiquidityUsage, Pool, StakeAccount,
  TaxRecipient,
};
use crate::msg::InstantiateMsg;
use cosmwasm_std::{Addr, Coin, DepsMut, Env, MessageInfo, Storage, Uint128};
use cw_lib::models::Owner;
use cw_lib::utils::funds::has_funds;
use cw_storage_plus::{Item, Map};

pub const OWNER: Item<Owner> = Item::new("owner");
pub const POOL: Item<Pool> = Item::new("pool");
pub const CONFIG: Item<Config> = Item::new("config");
pub const STAKE_ACCOUNTS: Map<Addr, StakeAccount> = Map::new("accounts");
pub const BANK_ACCOUNTS: Map<Addr, BankAccount> = Map::new("bank_accounts");
pub const CLIENTS: Map<Addr, Client> = Map::new("clients");
pub const LEDGER: Map<u32, LedgerEntry> = Map::new("ledger");
pub const N_STAKE_ACCOUNTS: Item<u32> = Item::new("n_stake_accounts");
pub const N_LEDGER_ENTRIES: Item<u32> = Item::new("n_ledger_entries");
pub const N_CLIENTS: Item<u32> = Item::new("n_clients");
pub const TAX_RECIPIENTS: Map<Addr, TaxRecipient> = Map::new("tax_recipients");
pub const TAX_RATE: Item<u16> = Item::new("tax_rate");
pub const LIQUIDITY_USAGE: Map<Addr, LiquidityUsage> = Map::new("client_metadata");

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
  N_STAKE_ACCOUNTS.save(deps.storage, &0)?;
  N_CLIENTS.save(deps.storage, &0)?;
  N_LEDGER_ENTRIES.save(deps.storage, &0)?;
  Ok(())
}

pub fn load_stake_account(
  storage: &dyn Storage,
  addr: &Addr,
) -> ContractResult<StakeAccount> {
  if let Some(account) = STAKE_ACCOUNTS.may_load(storage, addr.clone())? {
    Ok(account)
  } else {
    Err(ContractError::StakeAccountNotFound)
  }
}

pub fn load_bank_account(
  storage: &dyn Storage,
  addr: &Addr,
) -> ContractResult<BankAccount> {
  if let Some(account) = BANK_ACCOUNTS.may_load(storage, addr.clone())? {
    Ok(account)
  } else {
    Err(ContractError::BankAccountNotFound)
  }
}

pub fn sync_account(
  storage: &mut dyn Storage,
  account: &mut StakeAccount,
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
  account: &mut StakeAccount,
) -> ContractResult<LedgerUpdates> {
  let n_entries = N_LEDGER_ENTRIES.load(storage)?;
  let mut updates = LedgerUpdates {
    zombie_entry_indices: vec![],
    updated_entries: vec![],
  };

  if n_entries > account.offset {
    for i_entry in account.offset..n_entries {
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
    account.offset = n_entries;
  }
  Ok(updates)
}

pub fn validate_and_update_client(
  storage: &mut dyn Storage,
  addr: &Addr,
  maybe_action: Option<&dyn Fn(&mut Client)>,
) -> ContractResult<Client> {
  Ok(
    CLIENTS.update(storage, addr.clone(), |maybe_client| -> ContractResult<_> {
      if let Some(mut client) = maybe_client {
        if client.is_suspended {
          return Err(ContractError::IsSuspended);
        } else {
          if let Some(action) = maybe_action {
            action(&mut client);
          }
          Ok(client)
        }
      } else {
        return Err(ContractError::ClientNotFound);
      }
    })?,
  )
}

pub fn ensure_min_amount<T>(
  amount: T,
  min_amount: T,
) -> ContractResult<()>
where
  T: std::cmp::PartialOrd,
{
  if amount < min_amount {
    Err(ContractError::InsufficientAmount)
  } else {
    Ok(())
  }
}

pub fn ensure_has_funds(
  funds: &Vec<Coin>,
  denom: &String,
  amount: Uint128,
) -> ContractResult<()> {
  if !has_funds(funds, amount, denom) {
    // insufficient funds
    return Err(ContractError::InsufficientFunds);
  }
  Ok(())
}
