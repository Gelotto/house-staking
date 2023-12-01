use std::collections::HashMap;

use crate::{
  error::{ContractError, ContractResult},
  models::{AccountTokenAmount, Client, Config, HouseEvent, Pool, RateLimitConfig, Usage},
  msg::Job,
  state::{
    amortize, ensure_client_not_rate_limited, ensure_has_funds, ensure_min_amount, load_client,
    suspend_client, upsert_ledger_entry, validate_address, CLIENTS, CLIENT_EXECUTION_COUNTS,
    CONFIG, EVENTS, MAX_EVENT_QUEUE_SIZE, POOL, USAGE,
  },
  utils::mul_pct,
};
use cosmwasm_std::{
  attr, Addr, Api, BlockInfo, DepsMut, Env, MessageInfo, Response, Storage, Uint128, Uint64,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_transfer_from_msg, build_send_submsg},
};

enum RateLimitEvent {
  Throttled,
  Triggered,
}

pub fn process_many(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  jobs: Vec<Job>,
  maybe_cw20_sender: Option<Addr>,
) -> ContractResult<Response> {
  let client_addr = maybe_cw20_sender.unwrap_or_else(|| info.sender.clone());
  let config = CONFIG.load(deps.storage)?;
  let mut pool = POOL.load(deps.storage)?;
  let mut client = load_client(deps.storage, &client_addr)?;
  let mut incoming_totals: HashMap<Addr, Uint128> = HashMap::with_capacity(jobs.len());
  let mut outgoing_totals: HashMap<Addr, Uint128> = HashMap::with_capacity(jobs.len());
  let mut resp = Response::new().add_attributes(vec![attr("action", "process_many")]);

  for job in jobs.iter() {
    let is_rate_limited = process(
      deps.api,
      deps.storage,
      &env,
      &client_addr,
      &mut client,
      &mut pool,
      &config,
      job.initiator.clone(),
      job.incoming.clone(),
      job.outgoing.clone(),
    )?;

    // Send refund and continue if rate limited
    if is_rate_limited {
      if let Some(incoming) = &job.incoming {
        if !incoming.amount.is_zero() {
          resp = resp
            .add_attribute("rate_limited", "true")
            .add_submessage(build_send_submsg(
              &job.initiator,
              incoming.amount,
              &pool.token,
            )?);
        }
      }
      continue;
    }

    if let Some(incoming) = &job.incoming {
      if !incoming.amount.is_zero() {
        incoming_totals.insert(
          incoming.address.clone(),
          *incoming_totals
            .get(&incoming.address)
            .unwrap_or(&Uint128::zero())
            + incoming.amount,
        );
      }
    }

    if let Some(outgoing) = &job.outgoing {
      if !outgoing.amount.is_zero() {
        outgoing_totals.insert(
          outgoing.address.clone(),
          *outgoing_totals
            .get(&outgoing.address)
            .unwrap_or(&Uint128::zero())
            + outgoing.amount,
        );
      }
    }
  }

  // Update client exection counter
  CLIENT_EXECUTION_COUNTS.update(
    deps.storage,
    client_addr.clone(),
    |maybe_n| -> Result<_, ContractError> {
      Ok(maybe_n.unwrap_or_default() + Uint64::from(jobs.len() as u64))
    },
  )?;

  let mut total_incoming_amount = Uint128::zero();
  let mut total_outgoing_amount = Uint128::zero();

  // Transfer all incoming to house, regardless of whether there's any outgoing
  // amount, because if there is indeed an outgoing amount, then we will
  // transfer it from the house's own balance after incrementing it here.
  for (from_addr, amount) in incoming_totals.iter() {
    total_incoming_amount += amount;
    match &pool.token {
      Token::Native { denom } => {
        ensure_has_funds(&info.funds, denom, *amount)?;
      },
      Token::Cw20 { address } => {
        resp = resp.add_message(build_cw20_transfer_from_msg(
          &from_addr,
          &env.contract.address,
          address,
          *amount,
        )?)
      },
    }
  }

  // Send outgoing amounts
  for (to_addr, amount) in outgoing_totals.iter() {
    total_outgoing_amount += amount;
    resp = resp.add_submessage(build_send_submsg(to_addr, *amount, &pool.token)?)
  }

  Ok(resp.add_attributes(vec![
    attr("jobs", jobs.len().to_string()),
    attr("outgoing", total_outgoing_amount.to_string()),
    attr("incoming", total_incoming_amount.to_string()),
  ]))
}

pub fn process_one(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  initiator: Addr,
  maybe_incoming: Option<AccountTokenAmount>,
  maybe_outgoing: Option<AccountTokenAmount>,
  maybe_cw20_sender: Option<Addr>,
) -> ContractResult<Response> {
  Ok(process_many(
    deps,
    env,
    info,
    vec![Job {
      initiator,
      incoming: maybe_incoming,
      outgoing: maybe_outgoing,
    }],
    maybe_cw20_sender,
  )?)
}

fn process(
  api: &dyn Api,
  storage: &mut dyn Storage,
  env: &Env,
  client_address: &Addr,
  client: &mut Client,
  pool: &mut Pool,
  config: &Config,
  initiator: Addr,
  maybe_incoming: Option<AccountTokenAmount>,
  maybe_outgoing: Option<AccountTokenAmount>,
) -> ContractResult<bool> {
  validate_address(api, &initiator)?;
  ensure_client_not_rate_limited(client)?;

  // Abort if nothings being sent or received
  if maybe_incoming.is_none() && maybe_outgoing.is_none() {
    amortize(storage, api)?;
    return Ok(false);
  }

  // Check if outgoing amount exceeds budget. If not, disconnect the client if
  // the budget is all used up at the end.
  let mut suspend_after_use = false;
  if let Some(budget) = &client.config.budget {
    if let Some(outgoing) = &maybe_outgoing {
      if outgoing.amount > *budget {
        return Err(ContractError::BudgetExceeded);
      } else {
        let remaining_budget = *budget - outgoing.amount;
        client.config.budget = Some(remaining_budget);
        suspend_after_use = remaining_budget.is_zero();
      }
    }
  }

  // Get or default the incoming AccountTokenAmount so we don't have to deal
  // with the Option value going forward.
  let incoming = maybe_incoming.unwrap_or_else(|| AccountTokenAmount {
    address: client_address.clone(),
    amount: Uint128::zero(),
  });

  incoming.validate(api)?;

  let mut is_rate_limit_triggered = false;

  // Apply rate limiting if no budget is set for the client.
  if client.config.budget.is_none() {
    // Apply rate limiting at the client contract level
    if let Some(event) = throttle(
      storage,
      api,
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
          CLIENTS.save(storage, client_address.clone(), &client)?;
          EVENTS.push_front(
            storage,
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
        storage,
        api,
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
              storage,
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
  }

  // Ensure that the events buffer is capped at max size
  while EVENTS.len(storage)? > MAX_EVENT_QUEUE_SIZE {
    EVENTS.pop_back(storage)?;
  }

  if is_rate_limit_triggered {
    return Ok(true);
  }

  // Take earnings and/or send payment
  if let Some(outgoing) = maybe_outgoing {
    outgoing.validate(api)?;
    if outgoing.amount != incoming.amount {
      if outgoing.amount > incoming.amount {
        // Pay out of house to the outgoing account.
        let payment = outgoing.amount - incoming.amount;
        send(api, storage, pool, client, payment, client_address)?;
      } else {
        // Take payment from incoming account.
        let revenue = incoming.amount - outgoing.amount;
        receive(api, storage, pool, client, revenue, &config, client_address)?;
      };
    }
  } else if !incoming.amount.is_zero() {
    // There's only incoming, no outgoing, so the house takes revenue.
    receive(
      api,
      storage,
      pool,
      client,
      incoming.amount,
      &config,
      client_address,
    )?;
  }

  // Suspend client when it has used all of its remaining budget
  if suspend_after_use {
    suspend_client(storage, client_address)?;
  }

  Ok(false)
}

fn throttle(
  storage: &mut dyn Storage,
  _api: &dyn Api,
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

      // Do checks BEFORE incrementing usage to check if ALREADY rate limited
      if record.spent > record.added && (record.spent - record.added) >= spending_threshold {
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
  api: &dyn Api,
  storage: &mut dyn Storage,
  pool: &mut Pool,
  client: &mut Client,
  payment: Uint128,
  client_addr: &Addr,
) -> ContractResult<()> {
  ensure_min_amount(payment, Uint128::one())?;

  upsert_ledger_entry(storage, &pool, Uint128::zero(), Uint128::zero(), payment)?;

  // Increment client's total expenditure, subtracting from pool's liquidity.
  client.expense += payment;
  pool.liquidity -= payment;

  POOL.save(storage, &pool)?;
  CLIENTS.save(storage, client_addr.clone(), &client)?;

  amortize(storage, api)?;

  Ok(())
}

fn receive(
  api: &dyn Api,
  storage: &mut dyn Storage,
  pool: &mut Pool,
  client: &mut Client,
  revenue: Uint128,
  config: &Config,
  client_addr: &Addr,
) -> ContractResult<()> {
  ensure_min_amount(revenue, Uint128::one())?;

  let tax = mul_pct(revenue, config.tax_rate);
  let revenue_post_tax = revenue - tax;
  let delta_revenue = mul_pct(revenue_post_tax, config.restake_rate.into());
  let delta_dividends = revenue_post_tax - delta_revenue;

  upsert_ledger_entry(
    storage,
    &pool,
    delta_revenue,
    delta_dividends,
    Uint128::zero(),
  )?;

  // increment aggregate pool totals
  pool.liquidity += delta_revenue;
  pool.dividends += delta_dividends;
  pool.taxes += tax;

  client.revenue += revenue;

  POOL.save(storage, &pool)?;
  CLIENTS.save(storage, client_addr.clone(), &client)?;

  amortize(storage, api)?;

  Ok(())
}
