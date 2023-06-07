use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, BlockInfo, Timestamp, Uint128, Uint64};
use cw_lib::models::Token;

use crate::{error::ContractError, state::validate_address};

#[cw_serde]
pub struct Config {
  pub restake_rate: Uint128,
  pub tax_rate: Uint128,
  pub unbonding_seconds: Uint64,
  pub account_rate_limit: RateLimitConfig,
  pub default_client_rate_limit: RateLimitConfig,
}

#[cw_serde]
pub struct RateLimitConfig {
  pub interval_seconds: Uint64,
  pub max_pct_change: Uint128,
}

#[cw_serde]
pub enum Actor {
  Account,
  Client(Addr),
}

#[cw_serde]
pub enum HouseEvent {
  ClientRateLimitTriggered {
    block: BlockInfo,
    initiator: Addr,
    client: Addr,
  },
  AccountRateLimitTriggered {
    block: BlockInfo,
    client: Addr,
    initiator: Addr,
  },
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
pub struct StakeAccount {
  pub address: Option<Addr>,
  pub is_suspended: Option<bool>,
  pub delegation: Uint128,
  pub dividends: Uint128,
  pub liquidity: Uint128,
  pub unbonding: Option<UnbondingInfo>,
  pub seq_no: Uint128,
}

#[cw_serde]
pub struct BankAccount {
  pub address: Option<Addr>,
  pub balance: Uint128,
}

#[cw_serde]
pub struct UnbondingInfo {
  pub amount: Uint128,
  pub time: Timestamp,
}

#[cw_serde]
pub struct ClientConfig {
  pub name: Option<String>,
  pub description: Option<String>,
  pub url: Option<String>,
  pub budget: Option<Uint128>,
  pub rate_limit: RateLimitConfig,
}

#[cw_serde]
pub struct Client {
  pub address: Option<Addr>,
  pub config: ClientConfig,
  pub connected_at: Timestamp,
  pub is_suspended: bool,
  pub revenue: Uint128,
  pub expense: Uint128,
}

#[cw_serde]
pub struct LedgerEntry {
  pub liquidity: Uint128,
  pub delegation: Uint128,
  pub delta_revenue: Uint128,
  pub delta_dividends: Uint128,
  pub delta_loss: Uint128,
  pub ref_count: u32,
  pub tag: Uint128,
}

pub struct LedgerUpdates {
  pub zombie_entry_indices: Vec<u128>,
  pub updated_entries: Vec<(u128, LedgerEntry)>,
}

#[cw_serde]
pub struct TaxRecipient {
  pub address: Option<Addr>,
  pub pct: Uint128,
  pub name: Option<String>,
  pub description: Option<String>,
  pub url: Option<String>,
}

#[cw_serde]
pub struct LiquidityUsage {
  pub initial_liquidity: Uint128,
  pub total_amount: Uint128,
  pub time: Timestamp,
  pub height: Uint64,
}

#[cw_serde]
pub struct Usage {
  pub start_liquidity: Uint128,
  pub start_time: Timestamp,
  pub prev_height: Uint64,
  pub spent: Uint128,
  pub added: Uint128,
}

#[cw_serde]
pub struct AccountTokenAmount {
  pub address: Addr,
  pub amount: Uint128,
}

impl AccountTokenAmount {
  pub fn new(
    address: &Addr,
    amount: Uint128,
  ) -> Self {
    Self {
      address: address.clone(),
      amount,
    }
  }

  pub fn validate(
    &self,
    api: &dyn Api,
  ) -> Result<(), ContractError> {
    validate_address(api, &self.address)?;
    Ok(())
  }
}

impl StakeAccount {
  pub fn new(
    delegation: Uint128,
    seq_no: Uint128,
  ) -> Self {
    Self {
      seq_no,
      delegation,
      liquidity: delegation,
      dividends: Uint128::zero(),
      address: None,
      unbonding: None,
      is_suspended: Some(false),
    }
  }
}

impl Client {
  pub fn new(
    connected_at: Timestamp,
    address: Option<Addr>,
    budget: Option<Uint128>,
    name: Option<String>,
    description: Option<String>,
    url: Option<String>,
    rate_limit: RateLimitConfig,
  ) -> Self {
    Self {
      address,
      connected_at,
      expense: Uint128::zero(),
      revenue: Uint128::zero(),
      is_suspended: false,
      config: ClientConfig {
        name,
        rate_limit,
        description,
        url,
        budget,
      },
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
