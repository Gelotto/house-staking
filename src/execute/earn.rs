use crate::{
  error::ContractResult,
  models::LedgerEntry,
  state::{CONFIG, LEDGER, N_ACCOUNTS, N_LEDGER_ENTRIES, POOL, TAX_RATE},
  utils::{increment, mul_pct},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn earn(
  deps: DepsMut,
  _env: Env,
  _info: MessageInfo,
  revenue: Uint128,
) -> ContractResult<Response> {
  let action = "earn";

  let mut pool = POOL.load(deps.storage)?;
  let restake_rate = CONFIG.load(deps.storage)?.restake_rate;
  let i_snapshot = increment(deps.storage, &N_LEDGER_ENTRIES, 1)? - 1;
  let tax = mul_pct(revenue, TAX_RATE.load(deps.storage)?.into());
  let restaked_revenue = mul_pct(revenue - tax, restake_rate.into());

  let snapshot = LedgerEntry {
    ref_count: N_ACCOUNTS.load(deps.storage)?,
    liquidity: pool.liquidity,
    delegation: pool.delegation,
    delta_loss: Uint128::zero(),
    delta_dividends: revenue - restaked_revenue,
    delta_revenue: restaked_revenue,
  };

  pool.liquidity += snapshot.delta_revenue;
  pool.dividends += snapshot.delta_dividends;
  pool.taxes += tax;

  LEDGER.save(deps.storage, i_snapshot, &snapshot)?;
  POOL.save(deps.storage, &pool)?;

  Ok(Response::new().add_attributes(vec![attr("action", action)]))
}
