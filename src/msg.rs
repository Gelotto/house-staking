use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128, Uint64};
use cw_lib::models::{Owner, Token};

use crate::models::{
  AccountTokenAmount, BankAccount, Client, ClientConfig, Config, HouseEvent, LedgerEntry, Pool,
  RateLimitConfig, StakeAccount, TaxRecipient,
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
  SetConfig { address: Addr, config: ClientConfig },
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
    initiator: Addr,
    incoming: Option<AccountTokenAmount>,
    outgoing: Option<AccountTokenAmount>,
  },
  Receive {
    revenue: Uint128,
  },
  SetConfig {
    config: Config,
  },
  SetOwner {
    owner: Owner,
  },
  SetTaxes {
    recipients: Vec<TaxRecipient>,
  },
  PayTaxes,
}

#[cw_serde]
pub enum QueryMsg {
  Client {
    address: Addr,
  },
  Accounts {
    cursor: Option<Addr>,
    limit: Option<u8>,
  },
  CanSpend {
    client: Addr,
    initiator: Addr,
    amount: Option<Uint128>,
  },
  Select {
    fields: Option<Vec<String>>,
    wallet: Option<Addr>,
  },
}
#[cw_serde]
pub enum MigrateMsg {
  NoOp {},
  V0_0_4 {},
  V0_0_5 {},
}

#[cw_serde]
pub struct Metadata {
  pub n_accounts: u32,
  pub n_unbonding: u32,
  pub n_clients: u32,
  pub n_ledger_entries: u32,
  pub ledger_entry_seq_no: Uint128,
}

#[cw_serde]
pub struct AccountView {
  pub stake: Option<StakeAccount>,
  pub bank: Option<BankAccount>,
  pub client: Option<Client>,
  pub is_suspended: bool,
}

#[cw_serde]
pub struct LedgerEntryView {
  pub seq_no: Uint128,
  pub entry: LedgerEntry,
}

#[cw_serde]
pub struct Totals {
  pub revenue: Uint128,
  pub expense: Uint128,
}

#[cw_serde]
pub struct ClientView {
  pub address: Addr,
  pub config: ClientConfig,
  pub connected_at: Timestamp,
  pub is_suspended: bool,
  pub revenue: Uint128,
  pub expense: Uint128,
  pub executions: Uint64,
}

impl ClientView {
  pub fn new(
    client: &Client,
    address: &Addr,
    executions: Uint64,
  ) -> Self {
    Self {
      executions,
      address: address.clone(),
      config: client.config.clone(),
      connected_at: client.connected_at,
      is_suspended: client.is_suspended,
      expense: client.expense,
      revenue: client.revenue,
    }
  }
}

#[cw_serde]
pub struct SelectResponse {
  pub owner: Option<Owner>,
  pub config: Option<Config>,
  pub clients: Option<Vec<ClientView>>,
  pub pool: Option<Pool>,
  pub totals: Option<Totals>,
  pub account: Option<AccountView>,
  pub taxes: Option<Vec<TaxRecipient>>,
  pub metadata: Option<Metadata>,
  pub events: Option<Vec<HouseEvent>>,
}

#[cw_serde]
pub struct CanSpendResponse {
  pub can_spend: bool,
}

#[cw_serde]
pub struct ClientResponse {
  pub client: Option<ClientView>,
}
