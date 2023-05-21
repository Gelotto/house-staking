use crate::{
  error::ContractResult,
  models::Account,
  state::{sync_account, ACCOUNTS, N_LEDGER_ENTRIES, POOL},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn stake(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  amount: Uint128,
) -> ContractResult<Response> {
  let action = "stake";
  let n_snapshots = N_LEDGER_ENTRIES.load(deps.storage)?;
  let mut account = ACCOUNTS
    .may_load(deps.storage, info.sender.clone())?
    .unwrap_or_else(|| Account::new(Uint128::zero(), n_snapshots));

  POOL.update(deps.storage, |mut pool| -> ContractResult<_> {
    pool.delegation += amount;
    pool.liquidity += amount;
    Ok(pool)
  })?;

  sync_account(deps.storage, &mut account)?;

  account.delegation += amount;
  account.liquidity += amount;

  ACCOUNTS.save(deps.storage, info.sender.clone(), &account)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
