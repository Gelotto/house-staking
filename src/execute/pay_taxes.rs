use crate::{
  error::ContractResult,
  state::{POOL, TAX_RECIPIENTS},
  utils::mul_pct,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Order, Response, SubMsg, Uint128};
use cw_lib::utils::funds::build_send_submsg;

pub fn pay_taxes(
  deps: DepsMut,
  _env: Env,
  _info: MessageInfo,
) -> ContractResult<Response> {
  let action = "pay_taxes";
  let mut transfer_submsgs: Vec<SubMsg> = Vec::with_capacity(1);
  let mut pool = POOL.load(deps.storage)?;

  // build the token transfer SubMsg for each tax recipient
  for result in TAX_RECIPIENTS.range(deps.storage, None, None, Order::Ascending) {
    if let Ok((addr, recipient)) = result {
      let amount = mul_pct(pool.taxes, recipient.pct.into());
      transfer_submsgs.push(build_send_submsg(&addr, amount, &pool.token)?);
      pool.taxes -= amount;
    }
  }

  // if there's any dust left over, just move it into dividends
  // and zero it out.
  if !pool.taxes.is_zero() {
    pool.dividends += pool.taxes;
    pool.taxes = Uint128::zero();
  }

  POOL.save(deps.storage, &pool)?;

  Ok(
    Response::new()
      .add_attributes(vec![attr("action", action)])
      .add_submessages(transfer_submsgs),
  )
}
