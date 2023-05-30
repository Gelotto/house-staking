use cosmwasm_std::{to_binary, Addr, Coin, StdResult, Uint128, WasmMsg};
use cw_lib::utils::funds::build_cw20_increase_allowance_msg;

use crate::msg::ExecuteMsg;

pub struct House {
  pub address: Addr,
}

impl House {
  pub fn new(addr: &Addr) -> Self {
    Self {
      address: addr.clone(),
    }
  }

  pub fn process(
    &self,
    source: &Addr,
    target: &Addr,
    revenue: Uint128,
    payment: Uint128,
    maybe_funds: Option<Vec<Coin>>,
    maybe_token_address: Option<Addr>,
  ) -> StdResult<Vec<WasmMsg>> {
    let mut msgs: Vec<WasmMsg> = Vec::with_capacity(2);

    // If the house uses a CW20 token, increase the house's spending allowance
    // so it may transfer the required tokens from the source to its account.
    if let Some(token_address) = maybe_token_address {
      msgs.push(build_cw20_increase_allowance_msg(
        &token_address,
        &self.address,
        revenue,
        None,
      )?);
    }

    // Build the house "process" message to process revenue & payment.
    msgs.push(WasmMsg::Execute {
      contract_addr: self.address.clone().into(),
      funds: maybe_funds.unwrap_or(vec![]),
      msg: to_binary(&ExecuteMsg::Process {
        source: source.clone(),
        target: target.clone(),
        revenue,
        payment,
      })?,
    });

    Ok(msgs)
  }
}
