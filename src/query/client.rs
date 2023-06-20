use cosmwasm_std::{Addr, Deps};

use crate::{
  error::ContractResult,
  msg::{ClientResponse, ClientView},
  state::{CLIENTS, CLIENT_EXECUTION_COUNTS},
  utils::require_valid_address,
};

pub fn query_client(
  deps: Deps,
  client_address: Addr,
) -> ContractResult<ClientResponse> {
  require_valid_address(deps.api, &client_address)?;
  let maybe_client = CLIENTS.may_load(deps.storage, client_address.clone())?;
  let executions = CLIENT_EXECUTION_COUNTS
    .load(deps.storage, client_address.clone())
    .unwrap_or_default();

  Ok(ClientResponse {
    client: match maybe_client {
      Some(client) => Some(ClientView::new(&client, &client_address, executions)),
      None => None,
    },
  })
}
