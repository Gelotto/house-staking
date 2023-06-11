use crate::{
  error::{ContractError, ContractResult},
  models::UnbondingInfo,
  state::{
    amortize, load_stake_account, sync_account, CONFIG, N_DELEGATION_MUTATIONS, POOL,
    STAKE_ACCOUNTS,
  },
  utils::increment,
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn unstake(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "unstake";
  let mut pool = POOL.load(deps.storage)?;
  let mut account = load_stake_account(deps.storage, &info.sender)?;

  sync_account(deps.storage, deps.api, &mut account, true)?;

  let total_amount = account.liquidity + account.dividends;

  if !total_amount.is_zero() {
    // abort if user is trying to unstake too soon after most recent prev attempt
    if let Some(mut info) = account.unbonding.clone() {
      let config = CONFIG.load(deps.storage)?;
      let time_since = env.block.time.seconds() - info.time.seconds();
      if time_since <= config.unbonding_seconds.u64() {
        return Err(ContractError::NotAuthorized {});
      }
      info.amount += total_amount;
      info.time = env.block.time;
    } else {
      account.unbonding = Some(UnbondingInfo {
        amount: total_amount,
        time: env.block.time,
      });
    }

    pool.liquidity -= account.liquidity;
    pool.dividends -= account.dividends;
    pool.delegation -= account.delegation;

    account.liquidity = Uint128::zero();
    account.dividends = Uint128::zero();
    account.delegation = Uint128::zero();
  }

  POOL.save(deps.storage, &pool)?;
  STAKE_ACCOUNTS.save(deps.storage, info.sender.clone(), &account)?;

  // increment the delegation mutation counter, which lets the process method
  // know that a new LedgerEntry should be created when nexted executed, instead
  // of updating the existing latest entry.
  increment(deps.storage, &N_DELEGATION_MUTATIONS, Uint128::one())?;

  amortize(deps.storage, deps.api)?;

  Ok(Response::new().add_attributes(vec![
    attr("action", action),
    attr("amount", total_amount.to_string()),
  ]))
}
