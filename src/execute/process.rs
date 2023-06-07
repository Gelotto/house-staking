use crate::{
  error::{ContractError, ContractResult},
  models::{AccountTokenAmount, Client, Config, HouseEvent, LedgerEntry, LiquidityUsage, Pool},
  state::{
    amortize, authorize_and_update_client, ensure_has_funds, ensure_min_amount, is_rate_limited,
    validate_address, CLIENTS, CONFIG, EVENTS, LEDGER, LEDGER_ENTRY_SEQ_NO, LIQUIDITY_USAGE,
    N_DELEGATION_MUTATIONS, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, POOL,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{
  attr, Addr, BlockInfo, DepsMut, Env, Event, MessageInfo, Response, Storage, Uint128, Uint64,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_transfer_from_msg, build_send_submsg},
};

pub fn process(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  initiator: Addr,
  maybe_incoming: Option<AccountTokenAmount>,
  maybe_outgoing: Option<AccountTokenAmount>,
) -> ContractResult<Response> {
  let pool = POOL.load(deps.storage)?;
  let config = CONFIG.load(deps.storage)?;

  validate_address(deps.api, &initiator)?;

  if is_rate_limited(
    deps.storage,
    &env.block,
    &config.account_rate_limit,
    &initiator,
    None,
  )? {
    return Err(ContractError::ClientSuspended);
  }

  if is_rate_limited(
    deps.storage,
    &env.block,
    &config.account_rate_limit,
    &info.sender,
    None,
  )? {
    return Err(ContractError::ClientSuspended);
  }

  let mut resp = Response::new().add_attributes(vec![attr("action", "process")]);

  // Abort if nothings being sent or received
  if maybe_incoming.is_none() && maybe_outgoing.is_none() {
    amortize(deps.storage)?;
    return Ok(resp);
  }

  // Get or default the incoming AccountTokenAmount so we don't have to deal
  // with the Option value going forward.
  let incoming = maybe_incoming.unwrap_or_else(|| AccountTokenAmount {
    address: info.sender.clone(),
    amount: Uint128::zero(),
  });

  incoming.validate(deps.api)?;

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
    outgoing.validate(deps.api)?;

    let mut is_rate_limit_triggered = false;

    if outgoing.amount != incoming.amount {
      // If the outgoing amount is greater than incoming, simply pay out the
      // difference unless we encounter a liquidity usage rate-limit.
      resp = if outgoing.amount > incoming.amount {
        let payment = outgoing.amount - incoming.amount;
        // Pay outgoing address. If make_payment returns None, it means that
        // either a client or account-level rate-limit was triggered.
        if let Some(resp) = make_payment(
          deps,
          env,
          info,
          payment,
          &initiator,
          &outgoing.address,
          &config,
          &resp,
        )? {
          // return response with payment submsg
          resp.add_attribute("rate_limit", "false")
        } else {
          // Rate limit triggered, so we now "refund" initiator if necessary.
          is_rate_limit_triggered = true;
          resp = resp.add_attribute("rate_limit", "true");
          if !incoming.amount.is_zero() {
            resp = resp.add_submessage(build_send_submsg(&initiator, incoming.amount, &pool.token)?)
          }
          resp
        }
      } else {
        // Add submsg to take incoming amount as house revenue
        let revenue = incoming.amount - outgoing.amount;
        take_revenue(deps, env, info, revenue, &config, &resp)?
      };
    }
    // Add submsg to response to send outgoing tokens
    if !(outgoing.amount.is_zero() || is_rate_limit_triggered) {
      resp = resp.add_submessage(build_send_submsg(
        &outgoing.address,
        outgoing.amount,
        &pool.token,
      )?);
    }
  } else if !incoming.amount.is_zero() {
    // There's only incoming, nothing outgoing, so the house takes revenue.
    take_revenue(deps, env, info, incoming.amount, &config, &resp)?;
  }

  Ok(resp)
}

pub fn make_payment(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  payment: Uint128,
  initiator: &Addr,
  recipient: &Addr,
  config: &Config,
  base_resp: &Response,
) -> ContractResult<Option<Response>> {
  ensure_min_amount(payment, Uint128::one())?;

  let resp = base_resp.clone();
  let mut pool = POOL.load(deps.storage)?;
  let mut client = authorize_and_update_client(deps.storage, &info.sender, None)?;

  // Suspend client if payment exceeds its liquidity usage limit. Rate-limiting
  // is applied both at client contract and individual account levels.
  let is_client_rate_limit_triggered = apply_rate_limit(
    deps.storage,
    &env.block,
    &pool,
    payment,
    info.sender.clone(),
    client.config.rate_limit.interval_seconds.into(),
    client.config.rate_limit.max_pct_change,
  )?;

  let is_account_rate_limit_triggered = apply_rate_limit(
    deps.storage,
    &env.block,
    &pool,
    payment,
    recipient.clone(),
    config.account_rate_limit.interval_seconds.into(),
    config.account_rate_limit.max_pct_change,
  )?;

  if is_client_rate_limit_triggered {
    EVENTS.push_front(
      deps.storage,
      &HouseEvent::ClientRateLimitTriggered {
        client: info.sender.clone(),
        initiator: initiator.clone(),
        block: env.block.clone(),
      },
    )?;
  }

  if is_account_rate_limit_triggered {
    EVENTS.push_front(
      deps.storage,
      &HouseEvent::AccountRateLimitTriggered {
        client: info.sender.clone(),
        initiator: initiator.clone(),
        block: env.block.clone(),
      },
    )?;
  }

  if EVENTS.len(deps.storage)? > 100 {
    EVENTS.pop_back(deps.storage)?;
  }

  // Apply rate limiting.
  let is_rate_limit_triggered = is_client_rate_limit_triggered || is_account_rate_limit_triggered;
  let maybe_resp = if !is_rate_limit_triggered {
    upsert_ledger_entry(
      deps.storage,
      &pool,
      Uint128::zero(),
      Uint128::zero(),
      payment,
    )?;
    // Increment client's total expenditure, subtracting from pool's liquidity.
    client.expense += payment;
    pool.liquidity -= payment;
    POOL.save(deps.storage, &pool)?;
    Some(resp.add_event(Event::new("pay").add_attribute("amount", payment.to_string())))
  } else {
    // Suspend the client if rate limiting was triggered.
    if is_client_rate_limit_triggered {
      client.is_suspended = true;
    }
    None
  };

  CLIENTS.save(deps.storage, info.sender.clone(), &client)?;

  amortize(deps.storage)?;

  Ok(maybe_resp)
}

fn apply_rate_limit(
  storage: &mut dyn Storage,
  block: &BlockInfo,
  pool: &Pool,
  payment: Uint128,
  addr: Addr,
  interval_secs: u64,
  pct: Uint128,
) -> ContractResult<bool> {
  let mut rate_limit_triggered = false;
  let time = block.time;
  let height = block.height;

  LIQUIDITY_USAGE.update(storage, addr.clone(), |maybe_record| -> ContractResult<_> {
    // Get or create a LiquidityUsage for the given address.
    let mut record = maybe_record.unwrap_or_else(|| LiquidityUsage {
      initial_liquidity: pool.liquidity,
      total_amount: Uint128::zero(),
      height: Uint64::zero(),
      time,
    });
    // Compute upper limit for total_payout. If total_payout reaches the threshold
    // then the rate limit comes into effect.
    let payout_threshold = mul_pct(record.initial_liquidity, Uint128::from(pct));

    // Get time between last LiquidityUsage reset and the current block.
    let time_delta = time.seconds() - record.time.seconds();

    // Determine if this tx is on a block subsequent to the existing block
    // height stored in the current LiquidityUsage record.
    let is_new_block = record.height.u64() < height;

    if time_delta >= interval_secs {
      // Reset the LiquidityUsage record if this tx comes after the required
      // time interval since the last time the existing record was reset.
      record.initial_liquidity = pool.liquidity;
      record.total_amount = Uint128::zero();
      record.time = time;
    }

    record.total_amount += payment;
    record.height = height.into();

    // Signal that rate limit is triggered but do not error out
    // so that the caller can take further action, like issuing
    // a refund, if required.
    if record.total_amount >= payout_threshold || !is_new_block {
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
  config: &Config,
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

  amortize(deps.storage)?;

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
