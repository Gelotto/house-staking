use crate::{
  error::ContractResult,
  models::{LedgerEntry, LiquidityUsage, Pool},
  state::{
    ensure_min_amount, validate_and_update_client, BANK_ACCOUNTS, CLIENTS, CONFIG, LEDGER,
    LIQUIDITY_USAGE, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, POOL,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{
  attr, Addr, Api, DepsMut, Env, MessageInfo, Response, Storage, Timestamp, Uint128,
};
use cw_lib::utils::funds::build_send_submsg;

pub fn pay(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  payment: Uint128,
  recipient: Addr,
) -> ContractResult<Response> {
  ensure_min_amount(payment, Uint128::one())?;

  let action = "pay";
  let config = CONFIG.load(deps.storage)?;

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
    client.config.rate_limit.interval_secs.into(),
    client.config.rate_limit.max_pct_change,
  )?;

  let account_rate_limit_triggered = is_rate_limited(
    deps.api,
    deps.storage,
    env.block.time,
    &pool,
    payment,
    recipient.clone(),
    config.account_rate_limit.interval_secs.into(),
    config.account_rate_limit.max_pct_change,
  )?;

  deps.api.debug(
    format!(
      ">>> client_rate_limit_triggered: {:?}",
      client_rate_limit_triggered
    )
    .as_str(),
  );

  deps.api.debug(
    format!(
      ">>> account_rate_limit_triggered: {:?}",
      account_rate_limit_triggered
    )
    .as_str(),
  );

  // build base response
  let mut resp = Response::new().add_attributes(vec![
    attr("action", action),
    attr("amount", payment.to_string()),
    attr(
      "rate_limit_triggered",
      client_rate_limit_triggered.to_string(),
    ),
  ]);

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
    if let Some(mut bank_account) = BANK_ACCOUNTS.may_load(deps.storage, recipient.clone())? {
      bank_account.balance += payment;
      BANK_ACCOUNTS.save(deps.storage, recipient.clone(), &bank_account)?;
    } else {
      resp = resp.add_submessage(build_send_submsg(&recipient, payment, &pool.token)?);
    }
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
  pct: u16,
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
