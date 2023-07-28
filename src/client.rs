use cosmwasm_std::{
  to_binary, Addr, Coin, Empty, QuerierWrapper, StdError, StdResult, Uint128, WasmMsg,
};
use cw_lib::{
  models::Token,
  utils::funds::{build_cw20_increase_allowance_msg, has_funds},
};

pub use crate::models::AccountTokenAmount;
use crate::msg::{CanSpendResponse, ExecuteMsg, QueryMsg};

pub struct House {
  pub address: Addr,
}

impl House {
  pub fn new(addr: &Addr) -> Self {
    Self {
      address: addr.clone(),
    }
  }

  pub fn can_spend(
    &self,
    querier: &QuerierWrapper<Empty>,
    client: &Addr,
    initiator: &Addr,
    amount: Option<Uint128>,
  ) -> StdResult<bool> {
    let resp = querier.query_wasm_smart::<CanSpendResponse>(
      self.address.clone(),
      &QueryMsg::CanSpend {
        client: client.clone(),
        initiator: initiator.clone(),
        amount,
      },
    )?;
    Ok(resp.can_spend)
  }

  pub fn receive(
    &self,
    token: Token,
    amount: Uint128,
    maybe_funds: Option<Vec<Coin>>,
  ) -> StdResult<Vec<WasmMsg>> {
    Ok(match token {
      Token::Native { denom } => {
        let funds = maybe_funds.unwrap_or_default();
        if has_funds(&funds, amount, &denom) {
          vec![WasmMsg::Execute {
            contract_addr: self.address.clone().into(),
            msg: to_binary(&ExecuteMsg::Receive { revenue: amount })?,
            funds,
          }]
        } else {
          return Err(StdError::GenericErr {
            msg: "insufficient funds".into(),
          });
        }
      },
      Token::Cw20 {
        address: cw20_address,
      } => {
        vec![
          build_cw20_increase_allowance_msg(&cw20_address, &self.address, amount, None)?,
          WasmMsg::Execute {
            contract_addr: self.address.clone().into(),
            msg: to_binary(&ExecuteMsg::Receive { revenue: amount })?,
            funds: vec![],
          },
        ]
      },
    })
  }

  pub fn process(
    &self,
    initiator: Addr,
    maybe_incoming: Option<AccountTokenAmount>,
    maybe_outgoing: Option<AccountTokenAmount>,
    maybe_funds: Option<Vec<Coin>>,
    maybe_token_address: Option<Addr>,
  ) -> StdResult<Vec<WasmMsg>> {
    let mut msgs: Vec<WasmMsg> = Vec::with_capacity(2);

    // If the house uses a CW20 token, increase the house's spending allowance
    // so it may transfer the required tokens from the source to its account.
    if let Some(token_address) = maybe_token_address {
      if let Some(incoming) = &maybe_incoming {
        msgs.push(build_cw20_increase_allowance_msg(
          &token_address,
          &self.address,
          incoming.amount,
          None,
        )?);
      }
    }

    // Build the house "process" message to process revenue & payment.
    msgs.push(WasmMsg::Execute {
      contract_addr: self.address.clone().into(),
      funds: maybe_funds.unwrap_or(vec![]),
      msg: to_binary(&ExecuteMsg::Process {
        initiator,
        incoming: maybe_incoming,
        outgoing: maybe_outgoing,
      })?,
    });

    Ok(msgs)
  }
}
