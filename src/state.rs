use crate::error::{ContractError, ContractResult};
use crate::models::{
  BankAccount, Client, Config, HouseEvent, LedgerEntry, LedgerUpdates, LiquidityUsage, Pool,
  RateLimitConfig, StakeAccount, TaxRecipient, Usage,
};
use crate::msg::InstantiateMsg;
use crate::utils::{decrement, mul_pct};
use cosmwasm_std::{Addr, Api, BlockInfo, Coin, Deps, DepsMut, Env, MessageInfo, Storage, Uint128};
use cw_acl::client::Acl;
use cw_lib::models::Owner;
use cw_lib::utils::funds::has_funds;
use cw_storage_plus::{Deque, Item, Map};

pub const MAX_EVENT_QUEUE_SIZE: u32 = 100;

pub const OWNER: Item<Owner> = Item::new("owner");
pub const POOL: Item<Pool> = Item::new("pool");
pub const CONFIG: Item<Config> = Item::new("config");
pub const STAKE_ACCOUNTS: Map<Addr, StakeAccount> = Map::new("stake_accounts");
pub const BANK_ACCOUNTS: Map<Addr, BankAccount> = Map::new("bank_accounts");
pub const CLIENTS: Map<Addr, Client> = Map::new("clients");
pub const LEDGER: Map<u128, LedgerEntry> = Map::new("ledger");
pub const LEDGER_ENTRY_SEQ_NO: Item<Uint128> = Item::new("ledger_entry_seq_no");
pub const N_LEDGER_ENTRIES: Item<u32> = Item::new("n_ledger_entries");
pub const N_STAKE_ACCOUNTS: Item<u32> = Item::new("n_stake_accounts");
pub const N_DELEGATION_MUTATIONS: Item<Uint128> = Item::new("n_delegation_mutations");
pub const N_CLIENTS: Item<u32> = Item::new("n_clients");
pub const TAX_RECIPIENTS: Map<Addr, TaxRecipient> = Map::new("tax_recipients");
pub const LIQUIDITY_USAGE: Map<Addr, LiquidityUsage> = Map::new("liquidity_usage");
pub const USAGE: Map<Addr, Usage> = Map::new("usage");
pub const MEMOIZATION_QUEUE: Deque<Addr> = Deque::new("memoization_queue");
pub const EVENTS: Deque<HouseEvent> = Deque::new("events");

// pub const : Deque<Addr> = Deque::new("memoization_queue");

/// Init contract state.
pub fn initialize(
  deps: DepsMut,
  _env: &Env,
  info: &MessageInfo,
  msg: &InstantiateMsg,
) -> ContractResult<()> {
  // TODO: validate
  OWNER.save(
    deps.storage,
    &msg
      .owner
      .clone()
      .unwrap_or_else(|| Owner::Address(info.sender.clone())),
  )?;
  POOL.save(deps.storage, &Pool::new(&msg.token))?;
  CONFIG.save(deps.storage, &msg.config)?;
  LEDGER_ENTRY_SEQ_NO.save(deps.storage, &Uint128::zero())?;
  N_STAKE_ACCOUNTS.save(deps.storage, &0)?;
  N_CLIENTS.save(deps.storage, &0)?;
  N_LEDGER_ENTRIES.save(deps.storage, &0)?;
  N_DELEGATION_MUTATIONS.save(deps.storage, &Uint128::zero())?;
  if let Some(recipients) = &msg.taxes {
    insert_tax_recipients(deps.storage, recipients)?;
  }
  Ok(())
}

pub fn insert_tax_recipients(
  storage: &mut dyn Storage,
  recipients: &Vec<TaxRecipient>,
) -> ContractResult<()> {
  for recipient in recipients.clone().iter_mut() {
    if let Some(addr) = recipient.address.clone() {
      recipient.address = None;
      TAX_RECIPIENTS.save(storage, addr.clone(), &recipient)?;
    }
  }
  Ok(())
}

/// Load a StakeAccount or return error.
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

/// Load a BankAccount or return error.
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

/// Load a Client or return error.
pub fn load_client(
  storage: &dyn Storage,
  addr: &Addr,
) -> ContractResult<Client> {
  if let Some(client) = CLIENTS.may_load(storage, addr.clone())? {
    Ok(client)
  } else {
    Err(ContractError::ClientNotFound)
  }
}

/// Compute an account's owed dividends, liquidity, losses, updating contract
/// state. The computed values are stored in the StakeAccount.
pub fn sync_account(
  storage: &mut dyn Storage,
  api: &dyn Api,
  account: &mut StakeAccount,
  is_claiming: bool,
) -> ContractResult<()> {
  // Empty ledger implies every account is fulled synced.
  if N_LEDGER_ENTRIES.load(storage)? == 0 {
    return Ok(());
  }

  // update the account's computed liquidity and dividends, returning any info
  // necessary to save changes to state below.
  let updates = sync_account_readonly(storage, api, account, is_claiming)?;

  // Save ledger entries that have been updated by sync readonly
  for (i_entry, entry) in updates.updated_entries.iter() {
    LEDGER.save(storage, *i_entry, entry)?;
  }

  // Decrease ledger entry count and remove zombie ledger entries.
  if !updates.zombie_entry_indices.is_empty() {
    decrement(
      storage,
      &N_LEDGER_ENTRIES,
      updates.zombie_entry_indices.len() as u32,
    )?;
    for i_entry in updates.zombie_entry_indices.iter() {
      LEDGER.remove(storage, *i_entry);
    }
  }
  Ok(())
}

/// Compute an account's owed dividends, liquidity, losses and return the data
/// strctures required to update contract state, generated by this "readonly"
/// procedure.
pub fn sync_account_readonly(
  storage: &dyn Storage,
  api: &dyn Api,
  account: &mut StakeAccount,
  is_claiming: bool,
) -> ContractResult<LedgerUpdates> {
  let mut current_seq_no = LEDGER_ENTRY_SEQ_NO.load(storage)?;

  let mut updates = LedgerUpdates {
    zombie_entry_indices: vec![],
    updated_entries: vec![],
  };

  if !current_seq_no.is_zero() && !is_claiming {
    current_seq_no -= Uint128::one();
  }

  api.debug(format!(">>> current_seq_no: {}", current_seq_no.u128()).as_str());
  api.debug(format!(">>> account.seq_no: {}", account.seq_no.u128()).as_str());

  if current_seq_no > account.seq_no {
    for i_entry in account.seq_no.u128()..current_seq_no.u128() {
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
    account.seq_no = current_seq_no;
  }
  Ok(updates)
}

/// Rotate through existing staking contracts, eagerly syncing them. This
/// amortizes the runtime of the claim and unstake functions, which would
/// otherwise need to iterate through every single ledger entry created
/// post-staking.
pub fn amortize(
  storage: &mut dyn Storage,
  api: &dyn Api,
) -> Result<(), ContractError> {
  let curr_seq_no = LEDGER_ENTRY_SEQ_NO.load(storage)?;
  for _ in 0..2 {
    if let Some(addr) = MEMOIZATION_QUEUE.pop_front(storage)? {
      if let Some(mut account) = STAKE_ACCOUNTS.may_load(storage, addr.clone())? {
        if account.unbonding.is_none() {
          MEMOIZATION_QUEUE.push_back(storage, &addr)?;
          if account.seq_no < curr_seq_no.into() {
            sync_account(storage, api, &mut account, false)?;
            STAKE_ACCOUNTS.save(storage, addr.clone(), &account)?;
            break;
          }
        }
      }
    }
  }
  Ok(())
}

/// Ensure the client contract is authorized and, if so, pass the client to a
/// callback. Returns and error if unauthorized.
pub fn authorize_and_update_client(
  storage: &mut dyn Storage,
  addr: &Addr,
  maybe_action: Option<&dyn Fn(&mut Client)>,
) -> ContractResult<Client> {
  Ok(
    CLIENTS.update(storage, addr.clone(), |maybe_client| -> ContractResult<_> {
      if let Some(mut client) = maybe_client {
        if client.is_suspended {
          return Err(ContractError::ClientSuspended);
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

pub fn is_rate_limited(
  storage: &dyn Storage,
  block: &BlockInfo,
  config: &RateLimitConfig,
  address: &Addr,
  additional_amount: Option<Uint128>,
) -> ContractResult<bool> {
  Ok(
    if let Some(usage) = LIQUIDITY_USAGE.may_load(storage, address.clone())? {
      let dt = block.time.seconds() - usage.time.seconds();
      let interval = config.interval_seconds.u64();
      if block.height == usage.height.u64() {
        true
      } else if dt >= interval {
        false
      } else {
        let max_pct_change = Uint128::from(config.max_pct_change);
        let thresh = mul_pct(usage.initial_liquidity, max_pct_change);
        usage.total_amount + additional_amount.unwrap_or_default() >= thresh
      }
    } else {
      false
    },
  )
}

pub fn validate_address(
  api: &dyn Api,
  addr: &Addr,
) -> Result<Addr, ContractError> {
  api
    .addr_validate(addr.as_str())
    .map_err(|_| ContractError::InvalidAddress)
}

/// Return error if amount is less than the given min amount.
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

/// Return error if funds vec is missing the given denom and amount.
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

/// Helper function that returns true if given wallet (principal) is authorized
/// by ACL to the given action.
pub fn ensure_sender_is_allowed(
  deps: &Deps,
  principal: &Addr,
  action: &str,
) -> Result<(), ContractError> {
  if !match OWNER.load(deps.storage)? {
    Owner::Address(addr) => *principal == addr,
    Owner::Acl(acl_addr) => {
      let acl = Acl::new(&acl_addr);
      acl.is_allowed(&deps.querier, principal, action)?
    },
  } {
    Err(ContractError::NotAuthorized {})
  } else {
    Ok(())
  }
}
