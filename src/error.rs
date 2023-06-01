use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
  #[error("{0}")]
  Std(#[from] StdError),

  #[error("NotAuthorized")]
  NotAuthorized {},

  #[error("InvalidAddress")]
  InvalidAddress,

  #[error("ValidationError")]
  ValidationError {},

  #[error("IsSuspended")]
  IsSuspended,

  #[error("ClientNotFound")]
  ClientNotFound,

  #[error("StakeAccountNotFound")]
  StakeAccountNotFound,

  #[error("BankAccountNotFound")]
  BankAccountNotFound,

  #[error("InsufficientFunds")]
  InsufficientFunds,

  #[error("InsufficientAmount")]
  InsufficientAmount,

  #[error("MissingSourceOrTarget")]
  MissingSourceOrTarget,
}

pub type ContractResult<T> = Result<T, ContractError>;
