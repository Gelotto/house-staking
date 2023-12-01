use cosmwasm_std::{
  to_binary, Addr, Coin, Empty, QuerierWrapper, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_lib::{models::Token, utils::funds::has_funds};

pub use crate::models::AccountTokenAmount;
pub use crate::msg::Job as HouseJob;
use crate::msg::{CanSpendResponse, Cw20ReceiveInnerMsg, ExecuteMsg, QueryMsg};

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
            msg: to_binary(&ExecuteMsg::ReceiveNative { amount })?,
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
        vec![WasmMsg::Execute {
          contract_addr: cw20_address.into(),
          msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: self.address.to_string(),
            msg: to_binary(&Cw20ReceiveInnerMsg::Receive {})?,
            amount,
          })?,
          funds: vec![],
        }]
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
  ) -> StdResult<WasmMsg> {
    // If the house uses a CW20 token, increase the house's spending allowance
    // so it may transfer the required tokens from the source to its account.
    Ok(if let Some(token_address) = maybe_token_address {
      WasmMsg::Execute {
        contract_addr: token_address.into(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Send {
          contract: self.address.clone().into(),
          amount: maybe_incoming
            .clone()
            .and_then(|ta| Some(ta.amount))
            .unwrap_or_default(),
          msg: to_binary(&Cw20ReceiveInnerMsg::Process {
            initiator,
            incoming: maybe_incoming,
            outgoing: maybe_outgoing,
          })?,
        })?,
      }
    } else {
      // Build the house "process" message to process revenue & payment.
      WasmMsg::Execute {
        contract_addr: self.address.clone().into(),
        funds: maybe_funds.unwrap_or(vec![]),
        msg: to_binary(&ExecuteMsg::Process {
          initiator,
          incoming: maybe_incoming,
          outgoing: maybe_outgoing,
        })?,
      }
    })
  }

  pub fn process_many(
    &self,
    jobs: Vec<HouseJob>,
    maybe_funds: Option<Vec<Coin>>,
    maybe_token_address: Option<Addr>,
  ) -> StdResult<WasmMsg> {
    Ok(if let Some(token_address) = maybe_token_address {
      WasmMsg::Execute {
        contract_addr: token_address.into(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Send {
          contract: self.address.clone().into(),
          amount: jobs
            .iter()
            .map(|j| -> Uint128 {
              j.to_owned()
                .incoming
                .and_then(|ta| Some(ta.amount))
                .unwrap_or(Uint128::zero())
            })
            .sum(),
          msg: to_binary(&Cw20ReceiveInnerMsg::ProcessMany(jobs))?,
        })?,
      }
    } else {
      WasmMsg::Execute {
        contract_addr: self.address.clone().into(),
        funds: maybe_funds.unwrap_or(vec![]),
        msg: to_binary(&ExecuteMsg::ProcessMany(jobs))?,
      }
    })
  }
}
