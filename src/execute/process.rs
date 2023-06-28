use crate::{
  error::{ContractError, ContractResult},
  models::{
    AccountTokenAmount, Client, Config, HouseEvent, LedgerEntry, Pool, RateLimitConfig, Usage,
  },
  state::{
    amortize, authorize_and_update_client, ensure_has_funds, ensure_min_amount, load_client,
    validate_address, CLIENTS, CLIENT_EXECUTION_COUNTS, CONFIG, EVENTS, LEDGER,
    LEDGER_ENTRY_SEQ_NO, MAX_EVENT_QUEUE_SIZE, N_DELEGATION_MUTATIONS, N_LEDGER_ENTRIES,
    N_STAKE_ACCOUNTS, POOL, USAGE,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{
  attr, Addr, Api, BlockInfo, DepsMut, Env, Event, MessageInfo, Response, Storage, Uint128, Uint64,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_transfer_from_msg, build_send_submsg},
};

enum RateLimitEvent {
  Throttled,
  Triggered,
}

pub fn process(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  initiator: Addr,
  maybe_incoming: Option<AccountTokenAmount>,
  maybe_outgoing: Option<AccountTokenAmount>,
) -> ContractResult<Response> {
  let client_address = &info.sender;
  let pool = POOL.load(deps.storage)?;
  let config = CONFIG.load(deps.storage)?;
  let mut client = load_client(deps.storage, &info.sender)?;
  let mut resp = Response::new().add_attributes(vec![attr("action", "process")]);

  validate_address(deps.api, &initiator)?;

  // Abort if nothings being sent or received
  if maybe_incoming.is_none() && maybe_outgoing.is_none() {
    amortize(deps.storage, deps.api)?;
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

  let mut is_rate_limit_triggered = false;

  // Apply rate limiting at the client contract level
  if let Some(event) = throttle(
    deps.storage,
    deps.api,
    &env.block,
    &pool,
    &incoming,
    &maybe_outgoing,
    &client.config.rate_limit,
    &client_address,
  )? {
    match event {
      RateLimitEvent::Throttled => return Err(ContractError::ClientSuspended),
      RateLimitEvent::Triggered => {
        is_rate_limit_triggered = true;
        client.is_suspended = true;
        CLIENTS.save(deps.storage, client_address.clone(), &client)?;
        EVENTS.push_front(
          deps.storage,
          &HouseEvent::ClientRateLimitTriggered {
            client: client_address.clone(),
            initiator: initiator.clone(),
            block: env.block.clone(),
          },
        )?;
      },
    }
  }

  // Apply rate limiting at the initiator account level
  if initiator != *client_address {
    if let Some(event) = throttle(
      deps.storage,
      deps.api,
      &env.block,
      &pool,
      &incoming,
      &maybe_outgoing,
      &config.account_rate_limit,
      &initiator,
    )? {
      match event {
        RateLimitEvent::Throttled => return Err(ContractError::AccountSuspended),
        RateLimitEvent::Triggered => {
          is_rate_limit_triggered = true;
          EVENTS.push_front(
            deps.storage,
            &HouseEvent::AccountRateLimitTriggered {
              client: client_address.clone(),
              initiator: initiator.clone(),
              block: env.block.clone(),
            },
          )?;
        },
      }
    }
  }

  // Ensure that the events buffer is capped at max size
  while EVENTS.len(deps.storage)? > MAX_EVENT_QUEUE_SIZE {
    EVENTS.pop_back(deps.storage)?;
  }

  // Update client exection counter
  CLIENT_EXECUTION_COUNTS.update(
    deps.storage,
    client_address.clone(),
    |maybe_n| -> Result<_, ContractError> { Ok(maybe_n.unwrap_or_default() + Uint64::one()) },
  )?;

  // Send full refund to initiator if any rate limit triggered.
  if is_rate_limit_triggered {
    if !incoming.amount.is_zero() {
      resp = resp.add_submessage(build_send_submsg(&initiator, incoming.amount, &pool.token)?);
    }
    return Ok(resp);
  }

  // Take earnings and/or send payment
  if let Some(outgoing) = maybe_outgoing {
    outgoing.validate(deps.api)?;

    if outgoing.amount != incoming.amount {
      resp = if outgoing.amount > incoming.amount {
        // Pay out of house to the outgoing account.
        let payment = outgoing.amount - incoming.amount;
        send(deps, info, payment, &resp)?
      } else {
        // Take payment from incoming account.
        let revenue = incoming.amount - outgoing.amount;
        receive(deps, env, info, revenue, &config, &resp)?
      };
    }
    // Add submsg to response to send outgoing tokens
    if !outgoing.amount.is_zero() {
      resp = resp.add_submessage(build_send_submsg(
        &outgoing.address,
        outgoing.amount,
        &pool.token,
      )?);
    }
  } else if !incoming.amount.is_zero() {
    // There's only incoming, no outgoing, so the house takes revenue.
    receive(deps, env, info, incoming.amount, &config, &resp)?;
  }

  Ok(resp)
}

fn throttle(
  storage: &mut dyn Storage,
  api: &dyn Api,
  block: &BlockInfo,
  pool: &Pool,
  incoming: &AccountTokenAmount,
  maybe_outgoing: &Option<AccountTokenAmount>,
  config: &RateLimitConfig,
  address: &Addr,
) -> ContractResult<Option<RateLimitEvent>> {
  let mut maybe_event = None;
  let time = block.time;
  let height = block.height;

  // no need to assess rate limit if there's nothing outgoing
  if maybe_outgoing.is_none() {
    return Ok(None);
  }

  USAGE.update(
    storage,
    address.clone(),
    |maybe_record| -> ContractResult<_> {
      // Get or create a LiquidityUsage for the given address.
      let mut record = maybe_record.unwrap_or_else(|| Usage {
        start_liquidity: pool.liquidity,
        start_time: time,
        spent: Uint128::zero(),
        added: Uint128::zero(),
        prev_height: Uint64::zero(),
      });

      // Get time between last LiquidityUsage reset and the current block.
      let time_delta = time.seconds() - record.start_time.seconds();

      if time_delta > config.interval_seconds.u64() {
        // Reset the LiquidityUsage record if this tx comes after the required
        // time interval since the last time the existing record was reset.
        record.start_liquidity = pool.liquidity;
        record.start_time = time;
        record.spent = Uint128::zero();
        record.added = Uint128::zero();
      }

      // Compute upper limit for total_payout. If total_payout reaches the threshold
      // then the rate limit comes into effect.
      let spending_threshold =
        mul_pct(record.start_liquidity, Uint128::from(config.max_pct_change));

      // Determine if this tx is on a block subsequent to the existing block
      // height stored in the current LiquidityUsage record.
      let is_same_block = record.prev_height.u64() == height;

      // Do checks BEFORE incrementing usage to check if ALREADY rate limited
      if (record.spent > record.added && (record.spent - record.added) >= spending_threshold)
        || is_same_block
      {
        api.debug(format!(">>> usage record: {:?}", record).as_str());
        api.debug(format!(">>> spending threshold: {:?}", spending_threshold.u128()).as_str());
        api.debug(format!(">>> is same block: {:?}", is_same_block).as_str());
        maybe_event = Some(RateLimitEvent::Throttled);
      }

      record.prev_height = height.into();
      record.added += incoming.amount;

      if let Some(outgoing) = maybe_outgoing {
        record.spent += outgoing.amount;
      }

      // Signal that rate limit is triggered but do not error out so that the
      // caller can take further action, like issuing a refund, if required.
      if maybe_event.is_none()
        && record.spent > record.added
        && (record.spent - record.added) >= spending_threshold
      {
        maybe_event = Some(RateLimitEvent::Triggered);
      }
      Ok(record)
    },
  )?;
  Ok(maybe_event)
}

fn send(
  deps: DepsMut,
  info: MessageInfo,
  payment: Uint128,
  base_resp: &Response,
) -> ContractResult<Response> {
  ensure_min_amount(payment, Uint128::one())?;

  let mut pool = POOL.load(deps.storage)?;
  let mut client = authorize_and_update_client(deps.storage, &info.sender, None)?;

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
  CLIENTS.save(deps.storage, info.sender.clone(), &client)?;

  amortize(deps.storage, deps.api)?;

  Ok(
    base_resp
      .clone()
      .add_event(Event::new("pay").add_attribute("amount", payment.to_string())),
  )
}

fn receive(
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

  amortize(deps.storage, deps.api)?;

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
