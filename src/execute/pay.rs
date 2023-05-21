use crate::{
  error::{ContractError, ContractResult},
  models::LedgerEntry,
  state::{
    ensure_min_amount, ensure_sender_is_valid_client, CLIENTS, LEDGER, N_ACCOUNTS,
    N_LEDGER_ENTRIES, POOL,
  },
  utils::increment,
};
use cosmwasm_std::{attr, Addr, DepsMut, Env, MessageInfo, Response, Storage, Uint128};
use cw_lib::utils::funds::build_send_submsg;

pub fn pay(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
  payment: Uint128,
  recipient: Addr,
) -> ContractResult<Response> {
  ensure_sender_is_valid_client(deps.storage, &info.sender)?;
  ensure_min_amount(payment, Uint128::one());

  let action = "pay";
  let i_snapshot = increment(deps.storage, &N_LEDGER_ENTRIES, 1)? - 1;
  let mut pool = POOL.load(deps.storage)?;
  let snapshot = LedgerEntry {
    ref_count: N_ACCOUNTS.load(deps.storage)?,
    liquidity: pool.liquidity,
    delegation: pool.delegation,
    delta_dividends: Uint128::zero(),
    delta_revenue: Uint128::zero(),
    delta_loss: payment,
  };

  pool.liquidity -= snapshot.delta_loss;

  LEDGER.save(deps.storage, i_snapshot, &snapshot)?;
  POOL.save(deps.storage, &pool)?;

  // TODO: send payment to recipient
  Ok(
    Response::new()
      .add_attributes(vec![attr("action", action)])
      .add_submessage(build_send_submsg(&recipient, payment, &pool.token)?),
  )
}
