use crate::error::ContractResult;
use crate::msg::{
  ClientMsg, CreditMsg, Cw20ReceiveInnerMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolMsg,
  QueryMsg,
};
use crate::query;
use crate::state::{self, ensure_expected_cw20_token_type, OWNER};
use crate::{execute, migrations};
use cosmwasm_std::{entry_point, from_binary, Addr};
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw_lib::models::Owner;

const CONTRACT_NAME: &str = "crates.io:sath";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  msg: InstantiateMsg,
) -> ContractResult<Response> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
  state::initialize(deps, &env, &info, &msg)?;
  Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  msg: ExecuteMsg,
) -> ContractResult<Response> {
  match msg {
    ExecuteMsg::SetConfig { config } => execute::set_config(deps, env, info, config),
    ExecuteMsg::SetOwner { owner } => execute::set_owner(deps, env, info, owner),
    ExecuteMsg::PayTaxes => execute::pay_taxes(deps, env, info),
    ExecuteMsg::SetTaxes { recipients } => execute::set_taxes(deps, env, info, recipients),
    ExecuteMsg::ReceiveNative { amount } => execute::receive(deps, env, info, amount, None),
    ExecuteMsg::Receive(msg) => {
      let Cw20ReceiveMsg {
        sender,
        amount,
        msg: inner_msg,
      } = msg;
      ensure_expected_cw20_token_type(deps.storage, &info.sender)?;
      let sender = deps.api.addr_validate(&sender)?;
      match from_binary::<Cw20ReceiveInnerMsg>(&inner_msg)? {
        Cw20ReceiveInnerMsg::Receive {} => execute::receive(deps, env, info, amount, Some(sender)),
        Cw20ReceiveInnerMsg::Stake {} => {
          execute::pool::stake(deps, env, info, amount, Some(sender))
        },
        Cw20ReceiveInnerMsg::Process {
          initiator,
          incoming,
          outgoing,
        } => execute::process_one(deps, env, info, initiator, incoming, outgoing, Some(sender)),
        Cw20ReceiveInnerMsg::ProcessMany(jobs) => {
          execute::process_many(deps, env, info, jobs, Some(sender))
        },
      }
    },
    ExecuteMsg::ProcessMany(jobs) => execute::process_many(deps, env, info, jobs, None),
    ExecuteMsg::Process {
      initiator,
      incoming,
      outgoing,
    } => execute::process_one(deps, env, info, initiator, incoming, outgoing, None),

    ExecuteMsg::Pool(msg) => match msg {
      PoolMsg::Stake { amount } => execute::pool::stake(deps, env, info, amount, None),
      PoolMsg::Unstake => execute::pool::unstake(deps, env, info),
      PoolMsg::Withdraw => execute::pool::withdraw(deps, env, info),
      PoolMsg::Claim => execute::pool::claim(deps, env, info),
    },

    ExecuteMsg::Client(msg) => match msg {
      ClientMsg::Connect(init_args) => execute::client::connect(deps, env, info, init_args),
      ClientMsg::Disconnect { address } => execute::client::disconnect(deps, env, info, address),
      ClientMsg::Suspend { address } => execute::client::suspend(deps, env, info, address),
      ClientMsg::Resume { address } => execute::client::resume(deps, env, info, address),
      ClientMsg::SetConfig { address, config } => {
        execute::client::set_client_config(deps, env, info, address, config)
      },
    },

    ExecuteMsg::Credit(msg) => match msg {
      CreditMsg::Deposit { amount } => execute::credit::deposit(deps, env, info, amount),
      CreditMsg::Withdraw { amount } => execute::credit::withdraw(deps, env, info, amount),
    },
  }
}

#[entry_point]
pub fn query(
  deps: Deps,
  env: Env,
  msg: QueryMsg,
) -> ContractResult<Binary> {
  Ok(match msg {
    QueryMsg::Select { fields, wallet } => to_binary(&query::select(deps, env, fields, wallet)?),
    QueryMsg::Client { address } => to_binary(&query::query_client(deps, address)?),
    QueryMsg::Accounts { cursor, limit } => to_binary(&query::accounts(deps, cursor, limit)?),
    QueryMsg::CanSpend {
      client,
      initiator,
      amount,
    } => to_binary(&query::can_spend(deps, env, client, initiator, amount)?),
  }?)
}

#[entry_point]
pub fn migrate(
  deps: DepsMut,
  _env: Env,
  msg: MigrateMsg,
) -> ContractResult<Response> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
  match msg {
    MigrateMsg::V0_0_4 {} => migrations::v0_0_4::migrate(deps),
    MigrateMsg::V0_0_5 {} => migrations::v0_0_5::migrate(deps),
    MigrateMsg::NoOp {} => Ok(Response::default()),
    MigrateMsg::SetOwner {} => {
      OWNER.save(
        deps.storage,
        &Owner::Acl(Addr::unchecked(
          "juno1nzru5lrdwywqaxmsns40nyh3rt8fav9h4kk6qcge59clk5wa4jyq5mxe6w",
        )),
      )?;
      Ok(Response::default())
    },
  }
}
