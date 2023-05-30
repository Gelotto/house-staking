use cosmwasm_std::{Addr, Deps};

use crate::{
  error::ContractResult, msg::ClientResponse, state::CLIENTS, utils::require_valid_address,
};

pub fn query_client(
  deps: Deps,
  client_address: Addr,
) -> ContractResult<ClientResponse> {
  require_valid_address(deps.api, &client_address)?;
  Ok(ClientResponse {
    client: CLIENTS.may_load(deps.storage, client_address.clone())?,
  })
}
