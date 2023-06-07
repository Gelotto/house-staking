use crate::{
  error::ContractResult,
  state::{ensure_sender_is_allowed, POOL, TAX_RECIPIENTS},
  utils::mul_pct,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Order, Response, SubMsg, Uint128};
use cw_lib::utils::funds::build_send_submsg;

pub fn pay_taxes(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "pay_taxes";

  ensure_sender_is_allowed(&deps.as_ref(), &info.sender, action)?;

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

  // if there's any dust left over, just move it into liquidity
  if !pool.taxes.is_zero() {
    pool.liquidity += pool.taxes;
    pool.taxes = Uint128::zero();
  }

  POOL.save(deps.storage, &pool)?;

  Ok(
    Response::new()
      .add_attributes(vec![attr("action", action)])
      .add_submessages(transfer_submsgs),
  )
}
