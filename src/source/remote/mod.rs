//! Remote source module for fetching secrets from external secret managers.
//!
//! This module provides the infrastructure for integrating with remote secret
//! management services like Doppler and AWS Secrets Manager.

mod adapter;
mod factory;
mod traits;

pub mod providers;

pub use adapter::RemoteSourceAdapter;
pub use factory::{RemoteSourceFactory, RemoteSourceFactoryFn};
pub use traits::*;
