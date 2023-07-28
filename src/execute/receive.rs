use crate::{
  error::{ContractError, ContractResult},
  models::RevenueStream,
  state::{
    amortize, ensure_has_funds, ensure_min_amount, upsert_ledger_entry, validate_address, CONFIG,
    POOL, STREAMS, TOTAL_STREAM_REVENUE,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128, Uint64};
use cw_lib::{models::Token, utils::funds::build_cw20_transfer_from_msg};

pub fn receive(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  revenue: Uint128,
) -> ContractResult<Response> {
  validate_address(deps.api, &info.sender)?;
  ensure_min_amount(revenue, Uint128::one())?;

  let config = CONFIG.load(deps.storage)?;
  let mut pool = POOL.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![
    attr("action", "receive"),
    attr("amount", revenue.to_string()),
  ]);

  // increment aggregate total revenue received through this RevenueStream
  STREAMS.update(
    deps.storage,
    info.sender.clone(),
    |maybe_source| -> Result<_, ContractError> {
      let mut stream = maybe_source.unwrap_or_else(|| RevenueStream::new(env.block.time, None));
      stream.execution_count += Uint64::one();
      stream.revenue += revenue;
      Ok(stream)
    },
  )?;

  // Transfer tokens to the house
  if !revenue.is_zero() {
    match &pool.token {
      Token::Native { denom } => {
        ensure_has_funds(&info.funds, denom, revenue)?;
      },
      Token::Cw20 { address } => {
        resp = resp.add_message(build_cw20_transfer_from_msg(
          &info.sender,
          &env.contract.address,
          address,
          revenue,
        )?)
      },
    }
  } else {
    return Err(ContractError::InsufficientAmount);
  }

  // increase aggregate total revenue received across all revenue streams
  increment(deps.storage, &TOTAL_STREAM_REVENUE, revenue)?;

  // Update pool and ledger to reflect new token balances
  let tax = mul_pct(revenue, config.tax_rate);
  let revenue_post_tax = revenue - tax;
  let delta_revenue = mul_pct(revenue_post_tax, config.restake_rate.into());
  let delta_dividends = revenue_post_tax - delta_revenue;

  upsert_ledger_entry(
    deps.storage,
    &pool,
    delta_revenue,
    delta_dividends,
    Uint128::zero(),
  )?;

  // increment aggregate pool totals
  pool.liquidity += delta_revenue;
  pool.dividends += delta_dividends;
  pool.taxes += tax;

  POOL.save(deps.storage, &pool)?;

  amortize(deps.storage, deps.api)?;

  Ok(resp)
}
