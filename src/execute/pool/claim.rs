use crate::{
  error::{ContractError, ContractResult},
  models::{Pool, StakeAccount},
  state::{load_stake_account, sync_account, POOL, STAKE_ACCOUNTS},
};
use cosmwasm_std::{attr, DepsMut, Env, MessageInfo, Response, Uint128};

pub fn claim(
  deps: DepsMut,
  _env: Env,
  info: MessageInfo,
) -> ContractResult<Response> {
  let action = "claim";
  let mut pool = POOL.load(deps.storage)?;
  let mut account = load_stake_account(deps.storage, &info.sender)?;

  if account.unbonding.is_some() {
    // user must first withdraw upon unbonding
    return Err(ContractError::NotAuthorized {});
  }

  sync_account(deps.storage, &mut account)?;

  let amount = calc_claim_amount(&account, &pool);

  account.liquidity -= amount;
  pool.liquidity -= amount;

  STAKE_ACCOUNTS.save(deps.storage, info.sender.clone(), &account)?;
  POOL.save(deps.storage, &pool)?;

  Ok(
    Response::new().add_attributes(vec![
      attr("action", action),
      attr("amount", amount.to_string()),
    ]), //.add_submessage(build_send_submsg(&info.sender, claim_amount, &pool.token)?),
  )
}

fn calc_claim_amount(
  account: &StakeAccount,
  pool: &Pool,
) -> Uint128 {
  let net_profit = account.liquidity - account.delegation;
  let house_net_worth = pool.liquidity;

  // compute a percentage that describes how much of the claimant's net profit
  // consistutes the total size of house liquidity.
  let pct_ownership = net_profit.multiply_ratio(Uint128::from(1000u32), house_net_worth);

  // if ownership were represented as a decimal percentage, from 0 - 1, the
  // claim amount would be computed as `min(0.9, net_worth * 5 / (1 + pct))`.
  net_profit
    .multiply_ratio(
      Uint128::from(5u32 * 1000u32),
      Uint128::from(1000u32) + pct_ownership,
    )
    .min(Uint128::from(900u32))
}
