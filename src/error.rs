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

  #[error("AccountSuspended")]
  AccountSuspended,

  #[error("ClientSuspended")]
  ClientSuspended,

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

  #[error("Unbonding")]
  Unbonding,

  #[error("NotUnstaked")]
  NotUnstaked,

  #[error("BudgetExceeded")]
  BudgetExceeded,
}

pub type ContractResult<T> = Result<T, ContractError>;
