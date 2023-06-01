use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_lib::models::{Owner, Token};

use crate::models::{
  AccountTokenAmount, BankAccount, Client, Config, Pool, RateLimitConfig, StakeAccount,
  TaxRecipient,
};

#[cw_serde]
pub struct InstantiateMsg {
  pub owner: Option<Owner>,
  pub taxes: Option<Vec<TaxRecipient>>,
  pub token: Token,
  pub config: Config,
}

#[cw_serde]
pub struct ClientInitArgs {
  pub address: Option<Addr>,
  pub name: Option<String>,
  pub description: Option<String>,
  pub url: Option<String>,
  pub budget: Option<Uint128>,
  pub rate_limit: Option<RateLimitConfig>,
}

#[cw_serde]
pub enum ClientMsg {
  Connect(ClientInitArgs),
  Disconnect { address: Addr },
  Suspend { address: Addr },
  Resume { address: Addr },
}

#[cw_serde]
pub enum PoolMsg {
  Stake { amount: Uint128 },
  Claim,
  Unstake,
  Withdraw,
}

#[cw_serde]
pub enum CreditMsg {
  Deposit { amount: Uint128 },
  Withdraw { amount: Option<Uint128> },
}

#[cw_serde]
pub enum ExecuteMsg {
  Client(ClientMsg),
  Pool(PoolMsg),
  Credit(CreditMsg),
  Process {
    incoming: Option<AccountTokenAmount>,
    outgoing: Option<AccountTokenAmount>,
  },
  SetConfig {
    config: Config,
  },
  PayTaxes,
}

#[cw_serde]
pub enum QueryMsg {
  Client {
    address: Addr,
  },
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct Metadata {
  pub n_accounts: u32,
  pub n_clients: u32,
  pub n_ledger_entries: u32,
}

#[cw_serde]
pub struct AccountView {
  pub stake: Option<StakeAccount>,
  pub bank: Option<BankAccount>,
  pub client: Option<Client>,
}

#[cw_serde]
pub struct SelectResponse {
  pub owner: Option<Owner>,
  pub config: Option<Config>,
  pub clients: Option<Vec<Client>>,
  pub pool: Option<Pool>,
  pub account: Option<AccountView>,
  pub taxes: Option<Vec<TaxRecipient>>,
  pub metadata: Option<Metadata>,
}

#[cw_serde]
pub struct ClientResponse {
  pub client: Option<Client>,
}
