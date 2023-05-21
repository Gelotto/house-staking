#[cfg(not(feature = "library"))]
use crate::error::ContractResult;
use crate::execute;
use crate::msg::{ClientMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolMsg, QueryMsg};
use crate::query;
use crate::state;
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;

const CONTRACT_NAME: &str = "crates.io:house-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  msg: ExecuteMsg,
) -> ContractResult<Response> {
  match msg {
    ExecuteMsg::Pool(msg) => match msg {
      PoolMsg::Stake { amount } => execute::pool::stake(deps, env, info, amount),
      PoolMsg::Unstake => execute::pool::unstake(deps, env, info),
      PoolMsg::Withdraw => execute::pool::withdraw(deps, env, info),
      PoolMsg::Claim => execute::pool::claim(deps, env, info),
    },
    ExecuteMsg::Client(msg) => match msg {
      ClientMsg::Connect { client } => execute::client::connect(deps, env, info, client),
      ClientMsg::Disconnect { client } => execute::client::disconnect(deps, env, info, client),
      ClientMsg::Suspend { client } => execute::client::suspend(deps, env, info, client),
      ClientMsg::Resume { client } => execute::client::resume(deps, env, info, client),
    },
    ExecuteMsg::Earn { revenue } => execute::earn(deps, env, info, revenue),
    ExecuteMsg::Pay { payment, recipient } => execute::pay(deps, env, info, payment, recipient),
    ExecuteMsg::SetConfig { config } => execute::set_config(deps, env, info, config),
    ExecuteMsg::PayTaxes => execute::pay_taxes(deps, env, info),
  }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(
  deps: Deps,
  _env: Env,
  msg: QueryMsg,
) -> ContractResult<Binary> {
  let result = match msg {
    QueryMsg::Select { fields, wallet } => to_binary(&query::select(deps, fields, wallet)?),
  }?;
  Ok(result)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(
  _deps: DepsMut,
  _env: Env,
  _msg: MigrateMsg,
) -> ContractResult<Response> {
  Ok(Response::default())
}
