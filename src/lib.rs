#[cfg(feature = "library")]
pub mod client;

#[cfg(not(feature = "library"))]
pub mod contract;

#[cfg(not(feature = "library"))]
pub mod execute;

#[cfg(not(feature = "library"))]
pub mod query;

pub mod error;
pub mod models;
pub mod msg;
pub mod state;
pub mod utils;
