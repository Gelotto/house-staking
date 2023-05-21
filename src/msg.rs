use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_lib::models::{Owner, Token};

use crate::models::{Account, Client, Config, Pool, TaxRecipient};

#[cw_serde]
pub struct InstantiateMsg {
  pub owner: Option<Owner>,
  pub taxes: Option<Vec<TaxRecipient>>,
  pub token: Token,
  pub config: Config,
}

#[cw_serde]
pub enum ClientMsg {
  Connect { client: Client },
  Disconnect { client: Addr },
  Suspend { client: Addr },
  Resume { client: Addr },
}

#[cw_serde]
pub enum PoolMsg {
  Stake { amount: Uint128 },
  Claim,
  Unstake,
  Withdraw,
}

#[cw_serde]
pub enum ExecuteMsg {
  Client(ClientMsg),
  Pool(PoolMsg),
  Earn { revenue: Uint128 },
  Pay { payment: Uint128, recipient: Addr },
  SetConfig { config: Config },
  PayTaxes,
}

#[cw_serde]
pub enum QueryMsg {
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct SelectResponse {
  pub owner: Option<Owner>,
  pub config: Option<Config>,
  pub pool: Option<Pool>,
  pub account: Option<Account>,
  pub taxes: Option<Vec<TaxRecipient>>,
}
