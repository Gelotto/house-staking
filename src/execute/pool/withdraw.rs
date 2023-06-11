use crate::{
  error::{ContractError, ContractResult},
  state::{amortize, load_stake_account, CONFIG, N_STAKE_ACCOUNTS, POOL, STAKE_ACCOUNTS},
  utils::decrement,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response};
use cw_lib::utils::funds::build_send_submsg;

pub fn withdraw(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "withdraw";
  let account = load_stake_account(deps.storage, &info.sender)?;
  let token = POOL.load(deps.storage)?.token;
  let config = CONFIG.load(deps.storage)?;

  let mut resp = Response::new().add_attributes(vec![attr("action", action)]);

  if let Some(unbonding) = account.unbonding {
    // if the unbonding period has been met, remove the StakeAccount and create a submsg
    // for transferring the sender's tokens to the sender.
    if env.block.time.nanos() > unbonding.time.seconds() + config.unbonding_seconds.u64() {
      // remove the staking account
      STAKE_ACCOUNTS.remove(deps.storage, info.sender.clone());
      decrement(deps.storage, &N_STAKE_ACCOUNTS, 1)?;
      resp = resp
        .add_attribute("amount", unbonding.amount.to_string())
        .add_submessage(build_send_submsg(&info.sender, unbonding.amount, &token)?);
    } else {
      // still unbonding
      return Err(ContractError::Unbonding);
    }
  } else {
    // haven't unstaked yet
    return Err(ContractError::NotUnstaked);
  }

  amortize(deps.storage, deps.api)?;

  Ok(resp)
}
