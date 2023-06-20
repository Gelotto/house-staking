use crate::{error::ContractResult, state::CLIENT_EXECUTION_COUNTS};
use cosmwasm_std::{Addr, DepsMut, Response, Uint64};

/// We added an "execution count" that records the number of times each client
/// contract executes the house's "process" function. Here, we initialize the
/// only existing client's count to an estimated value, based on mintscan's
/// "executes" counter.
pub fn migrate(deps: DepsMut) -> ContractResult<Response> {
  let mortal_koinbat_addr =
    Addr::unchecked("juno1awste6nsghdrlfakr2cf4q7fqg0k348lkzas538s5z2ggglmgddqnayj5c");

  CLIENT_EXECUTION_COUNTS.save(deps.storage, mortal_koinbat_addr, &Uint64::from(3971u64))?;

  Ok(Response::default())
}
