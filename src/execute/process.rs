use crate::{
  error::{ContractError, ContractResult},
  models::{AccountTokenAmount, Client, LedgerEntry, LiquidityUsage, Pool},
  state::{
    ensure_has_funds, ensure_min_amount, validate_and_update_client, BANK_ACCOUNTS, CLIENTS,
    CONFIG, LEDGER, LIQUIDITY_USAGE, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, POOL, TAX_RATE,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{
  attr, Addr, Api, DepsMut, Env, Event, MessageInfo, Response, Storage, Timestamp, Uint128,
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
  if maybe_incoming.is_none() && maybe_outgoing.is_none() {
    return Err(ContractError::MissingSourceOrTarget);
  }

  let pool = POOL.load(deps.storage)?;
  let incoming = maybe_incoming.unwrap_or_else(|| AccountTokenAmount {
    address: info.sender.clone(),
    amount: Uint128::zero(),
  });

  let mut resp = Response::new().add_attributes(vec![attr("action", "process")]);

  // transfer all income to house.
  // TODO: check if credit available from source
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

  // add or subtract liquidity from the house
  if let Some(outgoing) = maybe_outgoing {
    if outgoing.amount != incoming.amount {
      resp = if outgoing.amount > incoming.amount {
        // target account profits
        pay(
          deps,
          env,
          info,
          outgoing.amount - incoming.amount,
          &outgoing.address,
          &resp,
        )?
      } else {
        // the house profits
        earn(deps, env, info, incoming.amount - outgoing.amount, &resp)?
      };
    }
    // send any payment tokens to target address
    if !outgoing.amount.is_zero() {
      resp = resp.add_submessage(build_send_submsg(
        &outgoing.address,
        outgoing.amount,
        &pool.token,
      )?);
    }
  } else if !incoming.amount.is_zero() {
    // the house profits
    earn(deps, env, info, incoming.amount, &resp)?;
  }

  Ok(resp)
}

pub fn pay(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  payment: Uint128,
  recipient: &Addr,
  base_resp: &Response,
) -> ContractResult<Response> {
  ensure_min_amount(payment, Uint128::one())?;

  let config = CONFIG.load(deps.storage)?;

  let mut resp = base_resp.clone();
  let mut pool = POOL.load(deps.storage)?;
  let mut client = validate_and_update_client(deps.storage, &info.sender, None)?;

  // suspend the client if this payment has exceeded the global 24h liquidity
  // expenditure limit. rate limiting is applied both at the client contract
  // level and the individual user account level.
  let client_rate_limit_triggered = is_rate_limited(
    deps.api,
    deps.storage,
    env.block.time,
    &pool,
    payment,
    info.sender.clone(),
    client.config.rate_limit.interval_seconds.into(),
    client.config.rate_limit.max_pct_change,
  )?;

  let account_rate_limit_triggered = is_rate_limited(
    deps.api,
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
  if !(client_rate_limit_triggered || account_rate_limit_triggered) {
    // create and persist new ledger entry
    let i_entry = increment(deps.storage, &N_LEDGER_ENTRIES, 1)? - 1;
    let ref_count = N_STAKE_ACCOUNTS.load(deps.storage)?;
    let entry = LedgerEntry {
      ref_count,
      liquidity: pool.liquidity,
      delegation: pool.delegation,
      delta_revenue: Uint128::zero(),
      delta_dividends: Uint128::zero(),
      delta_loss: payment,
    };

    // increment client's running total expenditure while subtracting it from
    // the pool's global liquidity
    client.expenditure += payment;
    pool.liquidity -= payment;

    LEDGER.save(deps.storage, i_entry, &entry)?;
    POOL.save(deps.storage, &pool)?;

    // ensure the payment can be made and build msg to transfer tokens to
    // recipient if necessary
    // if let Some(mut bank_account) = BANK_ACCOUNTS.may_load(deps.storage, recipient.clone())? {
    //   bank_account.balance += payment;
    //   BANK_ACCOUNTS.save(deps.storage, recipient.clone(), &bank_account)?;
    // } else {
    //   resp = resp.add_submessage(build_send_submsg(&recipient, payment, &pool.token)?);
    // }
  } else if client_rate_limit_triggered {
    // we auto-suspend the client if rate limiting was triggered
    client.is_suspended = true;
  }

  CLIENTS.save(deps.storage, info.sender.clone(), &client)?;

  Ok(resp)
}

fn is_rate_limited(
  api: &dyn Api,
  storage: &mut dyn Storage,
  time: Timestamp,
  pool: &Pool,
  payment: Uint128,
  addr: Addr,
  interval_secs: u64,
  pct: Uint128,
) -> ContractResult<bool> {
  // suspend the client or account address if this payment has exceeded the
  // liquidity usage limit.
  let mut rate_limit_triggered = false;
  LIQUIDITY_USAGE.update(storage, addr.clone(), |maybe_record| -> ContractResult<_> {
    let mut record = maybe_record.unwrap_or_else(|| LiquidityUsage {
      initial_liquidity: pool.liquidity,
      agg_payout: Uint128::zero(),
      time,
    });

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

    api.debug(format!(">>> addr: {:?}", addr).as_str());
    api.debug(format!(">>> delta_t: {:?}", delta_secs).as_str());
    api.debug(format!(">>> interval_secs: {:?}", interval_secs).as_str());
    api.debug(format!(">>> total outlay: {:?}", record.agg_payout).as_str());
    api.debug(format!(">>> rate_limiting_thresh_amount: {:?}", thresh).as_str());

    Ok(record)
  })?;
  if rate_limit_triggered {
    return Ok(true);
  }
  Ok(false)
}

pub fn earn(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  revenue: Uint128,
  // from_address: Addr,
  base_resp: &Response,
) -> ContractResult<Response> {
  // let from_address = from_address.unwrap_or(info.sender.clone());

  ensure_min_amount(revenue, Uint128::one())?;
  validate_and_update_client(
    deps.storage,
    &info.sender,
    Some(&|client: &mut Client| client.revenue += revenue),
  )?;

  let mut pool = POOL.load(deps.storage)?;
  let resp = base_resp
    .clone()
    .add_event(Event::new("earn").add_attributes(vec![attr("amount", revenue.to_string())]));

  let config = CONFIG.load(deps.storage)?;
  let i_entry = increment(deps.storage, &N_LEDGER_ENTRIES, 1)? - 1;
  let tax = mul_pct(revenue, TAX_RATE.load(deps.storage)?.into());

  // TODO: move this into caller
  // // if an source address has been given, then we're taking payment from the
  // // corresponding BankAccount. Otherwise, we will transfer the revenue amount
  // // from info.sender.
  // if let Some(mut bank_account) = BANK_ACCOUNTS.may_load(deps.storage, from_address.clone())? {
  //   if revenue > bank_account.balance {
  //     // insufficient balance
  //     return Err(ContractError::NotAuthorized {});
  //   }
  //   bank_account.balance -= revenue;
  //   BANK_ACCOUNTS.save(deps.storage, from_address.clone(), &bank_account)?;
  // } else {
  //   // if we're here, we aren't taking payment from a BankAccount. Instead,
  //   // we're expecting tokens to be transferred to the house from the sender.
  //   match &pool.token {
  //     Token::Native { denom } => {
  //       ensure_has_funds(&info.funds, denom, revenue)?;
  //     },
  //     Token::Cw20 {
  //       address: cw20_token_address,
  //     } => {
  //       // add a cw20 transfer submsg to the resp
  //       resp = resp.add_submessage(build_cw20_transfer_from_submsg(
  //         &info.sender,
  //         &env.contract.address,
  //         cw20_token_address,
  //         revenue,
  //       )?);
  //     },
  //   }
  // }

  // create a new ledger entry
  let revenue_post_tax = revenue - tax;
  let delta_revenue = mul_pct(revenue_post_tax, config.restake_rate.into());
  let delta_dividends = revenue_post_tax - delta_revenue;
  let entry = LedgerEntry {
    ref_count: N_STAKE_ACCOUNTS.load(deps.storage)?,
    liquidity: pool.liquidity,
    delegation: pool.delegation,
    delta_loss: Uint128::zero(),
    delta_revenue,
    delta_dividends,
  };

  LEDGER.save(deps.storage, i_entry, &entry)?;

  // increment aggregate pool totals
  pool.liquidity += entry.delta_revenue;
  pool.dividends += entry.delta_dividends;
  pool.taxes += tax;

  POOL.save(deps.storage, &pool)?;

  Ok(resp)
}
