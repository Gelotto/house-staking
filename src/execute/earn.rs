use crate::{
  error::{ContractError, ContractResult},
  models::{Client, LedgerEntry},
  state::{
    ensure_has_funds, ensure_min_amount, ensure_sender_is_valid_client, BANK_ACCOUNTS, CONFIG,
    LEDGER, N_LEDGER_ENTRIES, N_STAKE_ACCOUNTS, POOL, TAX_RATE,
  },
  utils::{increment, mul_pct},
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response, Uint128};
use cw_lib::{models::Token, utils::funds::build_cw20_transfer_from_submsg};

pub fn earn(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  revenue: Uint128,
  from_address: Option<Addr>,
) -> ContractResult<Response> {
  let action = "earn";
  let from_address = from_address.unwrap_or(info.sender.clone());

  ensure_min_amount(revenue, Uint128::one())?;
  ensure_sender_is_valid_client(
    deps.storage,
    &info.sender,
    Some(&|client: &mut Client| client.revenue += revenue),
  )?;

  let mut pool = POOL.load(deps.storage)?;
  let mut resp = Response::new().add_attributes(vec![attr("action", action)]);

  let config = CONFIG.load(deps.storage)?;
  let i_entry = increment(deps.storage, &N_LEDGER_ENTRIES, 1)? - 1;
  let tax = mul_pct(revenue, TAX_RATE.load(deps.storage)?.into());

  // if an source address has been given, then we're taking payment from the
  // corresponding BankAccount. Otherwise, we will transfer the revenue amount
  // from info.sender.
  if let Some(mut bank_account) = BANK_ACCOUNTS.may_load(deps.storage, from_address.clone())? {
    if revenue > bank_account.balance {
      // insufficient balance
      return Err(ContractError::NotAuthorized {});
    }
    bank_account.balance -= revenue;
    BANK_ACCOUNTS.save(deps.storage, from_address.clone(), &bank_account)?;
  } else {
    // if we're here, we aren't taking payment from a BankAccount. Instead,
    // we're expecting tokens to be transferred to the house from the sender.
    match &pool.token {
      Token::Native { denom } => {
        ensure_has_funds(&info.funds, denom, revenue)?;
      },
      Token::Cw20 {
        address: cw20_token_address,
      } => {
        // add a cw20 transfer submsg to the resp
        resp = resp.add_submessage(build_cw20_transfer_from_submsg(
          &info.sender,
          &env.contract.address,
          cw20_token_address,
          revenue,
        )?);
      },
    }
  }

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
