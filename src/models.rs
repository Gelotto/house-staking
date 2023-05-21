use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_lib::models::Token;
use cw_utils::Duration;

#[cw_serde]
pub struct Config {
  pub restake_rate: u16,
  pub unbonding_period: Duration,
}

#[cw_serde]
pub struct Pool {
  pub token: Token,
  pub delegation: Uint128,
  pub liquidity: Uint128,
  pub dividends: Uint128,
  pub taxes: Uint128,
}

#[cw_serde]
pub struct Account {
  pub address: Option<Addr>,
  pub delegation: Uint128,
  pub liquidity: Uint128,
  pub dividends: Uint128,
  pub offset: u32,
}

#[cw_serde]
pub struct Client {
  pub address: Option<Addr>,
  pub name: Option<String>,
  pub description: Option<String>,
  pub url: Option<String>,
  pub allowance: Option<Uint128>,
  pub revenue: Uint128,
  pub expenditure: Uint128,
  pub is_suspended: bool,
}

#[cw_serde]
pub struct LedgerEntry {
  pub liquidity: Uint128,
  pub delegation: Uint128,
  pub delta_revenue: Uint128,
  pub delta_dividends: Uint128,
  pub delta_loss: Uint128,
  pub ref_count: u32,
}

pub struct LedgerUpdates {
  pub zombie_entry_indices: Vec<u32>,
  pub updated_entries: Vec<(u32, LedgerEntry)>,
}

#[cw_serde]
pub struct TaxRecipient {
  pub addr: Option<Addr>,
  pub pct: u16,
  pub name: Option<String>,
  pub description: Option<String>,
  pub url: Option<String>,
}

impl Account {
  pub fn new(
    delegation: Uint128,
    offset: u32,
  ) -> Self {
    Self {
      offset,
      delegation,
      liquidity: delegation,
      dividends: Uint128::zero(),
      address: None,
    }
  }
}

impl Client {
  pub fn new(
    address: Option<Addr>,
    allowance: Option<Uint128>,
    name: Option<String>,
    description: Option<String>,
    url: Option<String>,
  ) -> Self {
    Self {
      address,
      name,
      description,
      url,
      allowance,
      expenditure: Uint128::zero(),
      revenue: Uint128::zero(),
      is_suspended: false,
    }
  }
}

impl Pool {
  pub fn new(token: &Token) -> Self {
    Self {
      liquidity: Uint128::zero(),
      delegation: Uint128::zero(),
      dividends: Uint128::zero(),
      taxes: Uint128::zero(),
      token: token.clone(),
    }
  }
}
