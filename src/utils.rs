use cosmwasm_std::{Addr, Api, Storage, Uint128};
use cw_storage_plus::Item;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{ContractError, ContractResult};

pub fn increment<T>(
  storage: &mut dyn Storage,
  item: &Item<T>,
  increment: T,
) -> ContractResult<T>
where
  T: DeserializeOwned + Serialize + std::ops::Add<Output = T>,
{
  item.update(storage, |x| -> ContractResult<_> { Ok(x + increment) })
}

pub fn decrement<T>(
  storage: &mut dyn Storage,
  item: &Item<T>,
  increment: T,
) -> ContractResult<T>
where
  T: DeserializeOwned + Serialize + std::ops::Sub<Output = T>,
{
  item.update(storage, |x| -> ContractResult<_> { Ok(x - increment) })
}

pub fn mul_pct(
  total: Uint128,
  pct: Uint128,
) -> Uint128 {
  total.multiply_ratio(pct, Uint128::from(1_000_000u128))
}

pub fn require_valid_address(
  api: &dyn Api,
  addr: &Addr,
) -> ContractResult<Addr> {
  api
    .addr_validate(addr.as_str())
    .map_err(|_| ContractError::InvalidAddress)
}
