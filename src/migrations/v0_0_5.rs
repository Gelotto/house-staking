use crate::{
  error::ContractResult,
  state::{STREAMS, TOTAL_STREAM_REVENUE},
};
use cosmwasm_std::{DepsMut, Order, Response, Uint128};

/// Init total stream revenue.
pub fn migrate(deps: DepsMut) -> ContractResult<Response> {
  let mut revenue = Uint128::zero();
  for result in STREAMS.range(deps.storage, None, None, Order::Ascending) {
    let stream = result?.1;
    revenue += stream.revenue;
  }
  TOTAL_STREAM_REVENUE.save(deps.storage, &revenue)?;
  Ok(Response::default())
}
