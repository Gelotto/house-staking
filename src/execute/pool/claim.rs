use crate::{
  error::ContractResult,
  state::{load_stake_account, sync_account, POOL, STAKE_ACCOUNTS},
};
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, Uint128};
use cw_lib::utils::funds::build_send_submsg;

pub fn claim(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let mut pool = POOL.load(deps.storage)?;
  let mut account = load_stake_account(deps.storage, &info.sender)?;
  let mut resp = Response::new().add_attribute("action", "claim");

  sync_account(deps.storage, &mut account)?;

  if !account.dividends.is_zero() {
    resp = resp
      .add_attribute("amount", account.dividends.to_string())
      .add_submessage(build_send_submsg(
        &info.sender,
        account.dividends,
        &pool.token,
      )?);

    account.dividends = Uint128::zero();
    pool.dividends -= account.dividends;

    STAKE_ACCOUNTS.save(deps.storage, info.sender.clone(), &account)?;
    POOL.save(deps.storage, &pool)?;
  }

  Ok(resp)
}
