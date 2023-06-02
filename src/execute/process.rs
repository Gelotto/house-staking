use crate::{
  error::{ContractError, ContractResult},
  models::{AccountTokenAmount, Client, LedgerEntry, LiquidityUsage, Pool},
  state::{
    amortize, authorize_and_update_client, ensure_has_funds, ensure_min_amount, CLIENTS, CONFIG,
    LEDGER, LEDGER_ENTRY_SEQ_NO, LIQUIDITY_USAGE, N_DELEGATION_MUTATIONS, N_LEDGER_ENTRIES,
    N_STAKE_ACCOUNTS, POOL, TAX_RATE,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{
  attr, Addr, DepsMut, Env, Event, MessageInfo, Response, Storage, Timestamp, Uint128,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_transfer_from_msg, build_send_submsg},
};

pub fn process(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  maybe_incoming: Option<AccountTokenAmount>,
  maybe_outgoing: Option<AccountTokenAmount>,
) -> ContractResult<Response> {
  let mut resp = Response::new().add_attributes(vec![attr("action", "process")]);

  amortize(deps.storage)?;

  // abort if nothings being sent or received
  if maybe_incoming.is_none() && maybe_outgoing.is_none() {
    return Ok(resp);
  }

  let pool = POOL.load(deps.storage)?;

  // get or default the incoming AccountTokenAmount so we don't have to deal
  // with the Option value going forward.
  let incoming = maybe_incoming.unwrap_or_else(|| AccountTokenAmount {
    address: info.sender.clone(),
    amount: Uint128::zero(),
  });

  // Transfer all incoming to house, regardless of whether there's any outgoing
  // amount, because if there is indeed an outgoing amount, then we will
  // transfer it from the house's own balance after incrementing it here.
  if !incoming.amount.is_zero() {
    match &pool.token {
      Token::Native { denom } => {
        ensure_has_funds(&info.funds, denom, incoming.amount)?;
      },
      Token::Cw20 { address } => {
        resp = resp.add_message(build_cw20_transfer_from_msg(
          &incoming.address,
          &env.contract.address,
          address,
          incoming.amount,
        )?)
      },
    }
  }

  // Take earnings and/or send payment
  if let Some(outgoing) = maybe_outgoing {
    let mut rate_limited = false;
    if outgoing.amount != incoming.amount {
      resp = if outgoing.amount > incoming.amount {
        // Pay out to outgoing address
        let payment = outgoing.amount - incoming.amount;
        if let Some(resp) = make_payment(deps, env, info, payment, &outgoing.address, &resp)? {
          resp // return response with payment submsg
        } else {
          // Issue a refund to the sender from the house.
          rate_limited = true;
          resp.add_submessage(build_send_submsg(
            &incoming.address,
            incoming.amount,
            &pool.token,
          )?)
        }
      } else {
        // Add submsg to take incoming amount as house revenue
        let revenue = incoming.amount - outgoing.amount;
        take_revenue(deps, env, info, revenue, &resp)?
      };
    }
    // Add submsg to response to send outgoing tokens
    if !(outgoing.amount.is_zero() || rate_limited) {
      resp = resp.add_submessage(build_send_submsg(
        &outgoing.address,
        outgoing.amount,
        &pool.token,
      )?);
    }
  } else if !incoming.amount.is_zero() {
    // There's only incoming, nothing outgoing,
    // so the house just takes revenue.
    take_revenue(deps, env, info, incoming.amount, &resp)?;
  }

  Ok(resp)
}

pub fn make_payment(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  payment: Uint128,
  recipient: &Addr,
  base_resp: &Response,
) -> ContractResult<Option<Response>> {
  ensure_min_amount(payment, Uint128::one())?;

  let config = CONFIG.load(deps.storage)?;

  let mut resp = base_resp.clone();
  let mut pool = POOL.load(deps.storage)?;
  let mut client = authorize_and_update_client(deps.storage, &info.sender, None)?;

  // suspend the client if this payment has exceeded the global 24h liquidity
  // expenditure limit. rate limiting is applied both at the client contract
  // level and the individual user account level.
  let client_rate_limit_triggered = apply_rate_limit(
    deps.storage,
    env.block.time,
    &pool,
    payment,
    info.sender.clone(),
    client.config.rate_limit.interval_seconds.into(),
    client.config.rate_limit.max_pct_change,
  )?;

  let account_rate_limit_triggered = apply_rate_limit(
    deps.storage,
    env.block.time,
    &pool,
    payment,
    recipient.clone(),
    config.account_rate_limit.interval_seconds.into(),
    config.account_rate_limit.max_pct_change,
  )?;

  // build response
  resp = resp.add_event(Event::new("pay").add_attributes(vec![
    attr("amount", payment.to_string()),
    attr("is_rate_limited", client_rate_limit_triggered.to_string()),
  ]));

  // apply liquidity rate limiting: if recipient address has a BankAccount,
  // increment its balance; otherwise, send GLTO to the recipient address
  // directly.
  let maybe_resp = if !(client_rate_limit_triggered || account_rate_limit_triggered) {
    upsert_ledger_entry(
      deps.storage,
      &pool,
      Uint128::zero(),
      Uint128::zero(),
      payment,
    )?;
    // increment client's running total expenditure while subtracting it from
    // the pool's global liquidity
    client.expenditure += payment;
    pool.liquidity -= payment;
    POOL.save(deps.storage, &pool)?;
    Some(resp)
  } else {
    // we auto-suspend the client if rate limiting was triggered
    if client_rate_limit_triggered {
      client.is_suspended = true;
    }
    None
  };

  CLIENTS.save(deps.storage, info.sender.clone(), &client)?;

  Ok(maybe_resp)
}

fn apply_rate_limit(
  storage: &mut dyn Storage,
  time: Timestamp,
  pool: &Pool,
  payment: Uint128,
  addr: Addr,
  interval_secs: u64,
  pct: Uint128,
) -> ContractResult<bool> {
  let mut rate_limit_triggered = false;

  LIQUIDITY_USAGE.update(storage, addr.clone(), |maybe_record| -> ContractResult<_> {
    let mut record = maybe_record.unwrap_or_else(|| LiquidityUsage {
      initial_liquidity: pool.liquidity,
      agg_payout: Uint128::zero(),
      time,
    });

    // get time between the current and previous usage records
    let delta_secs = time.seconds() - record.time.seconds();

    // reset the record if this block is happens within the configured interval
    if delta_secs >= interval_secs {
      record.initial_liquidity = payment;
      record.agg_payout = payment;
    } else {
      record.agg_payout += payment;
    }

    record.time = time;

    // auto-suspend the client if this payment takes the client
    // past its interval allowed liquidity usage level.
    let thresh = mul_pct(record.initial_liquidity, Uint128::from(pct));

    if record.agg_payout >= thresh {
      rate_limit_triggered = true;
    }

    Ok(record)
  })?;
  if rate_limit_triggered {
    return Ok(true);
  }
  Ok(false)
}

pub fn take_revenue(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  revenue: Uint128,
  // from_address: Addr,
  base_resp: &Response,
) -> ContractResult<Response> {
  // let from_address = from_address.unwrap_or(info.sender.clone());

  ensure_min_amount(revenue, Uint128::one())?;
  authorize_and_update_client(
    deps.storage,
    &info.sender,
    Some(&|client: &mut Client| client.revenue += revenue),
  )?;

  let mut pool = POOL.load(deps.storage)?;
  let resp = base_resp
    .clone()
    .add_event(Event::new("earn").add_attributes(vec![attr("amount", revenue.to_string())]));

  let config = CONFIG.load(deps.storage)?;
  let tax = mul_pct(revenue, TAX_RATE.load(deps.storage)?.into());
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

  Ok(resp)
}

fn upsert_ledger_entry(
  storage: &mut dyn Storage,
  pool: &Pool,
  delta_revenue: Uint128,
  delta_dividends: Uint128,
  delta_loss: Uint128,
) -> Result<LedgerEntry, ContractError> {
  let n_entries = N_LEDGER_ENTRIES.load(storage)?;
  let seq_no = LEDGER_ENTRY_SEQ_NO.load(storage)?;
  let tag = N_DELEGATION_MUTATIONS.load(storage)?;

  // First, we try to increment the latest existing entry instead of making a
  // new one. we do this to keep the number of new entries created at a minimum
  // to help amortize the sync process.
  if n_entries > 0 {
    let i_prev = seq_no.u128() - 1u128;
    let mut prev_entry = LEDGER.load(storage, i_prev)?;
    if prev_entry.tag == tag {
      prev_entry.delta_revenue += delta_revenue;
      prev_entry.delta_dividends += delta_dividends;
      prev_entry.delta_loss += delta_loss;
      LEDGER.save(storage, i_prev, &prev_entry)?;
      return Ok(prev_entry);
    }
  }

  // if we weren't able to increment the latest existing entry,
  // we create a new one here.
  let entry = LedgerEntry {
    ref_count: N_STAKE_ACCOUNTS.load(storage)?,
    liquidity: pool.liquidity,
    delegation: pool.delegation,
    delta_loss,
    delta_revenue,
    delta_dividends,
    tag,
  };

  LEDGER.save(storage, seq_no.into(), &entry)?;

  increment(storage, &N_LEDGER_ENTRIES, 1)?;
  increment(storage, &LEDGER_ENTRY_SEQ_NO, Uint128::one())?;

  Ok(entry)
}
