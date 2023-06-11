use crate::{
  error::ContractResult,
  state::{load_stake_account, sync_account, N_DELEGATION_MUTATIONS, POOL, STAKE_ACCOUNTS},
  utils::increment,
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

  // TODO: instead of just is_unstaking, change to enum and have
  // SyncAction::Unstake, SyncAction::Claim, etc.
  sync_account(deps.storage, deps.api, &mut account, true)?;

  let claim_amount = account.dividends.clone();

  if !claim_amount.is_zero() {
    resp = resp
      .add_attribute("amount", claim_amount.to_string())
      .add_submessage(build_send_submsg(&info.sender, claim_amount, &pool.token)?);
  }

  pool.dividends -= claim_amount;
  account.dividends = Uint128::zero();

  POOL.save(deps.storage, &pool)?;
  STAKE_ACCOUNTS.save(deps.storage, info.sender.clone(), &account)?;

  increment(deps.storage, &N_DELEGATION_MUTATIONS, Uint128::one())?;

  Ok(resp)
}
