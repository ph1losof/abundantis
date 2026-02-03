//! Remote source provider implementations.
//!
//! Each provider module implements the RemoteSource trait for a specific
//! secret management service.

#[cfg(feature = "doppler")]
pub mod doppler;

#[cfg(feature = "aws")]
pub mod aws;

#[cfg(feature = "vault")]
pub mod vault;

#[cfg(feature = "infisical")]
pub mod infisical;

// Re-export provider types when features are enabled
#[cfg(feature = "doppler")]
pub use doppler::{DopplerSource, DopplerSourceFactory};

#[cfg(feature = "aws")]
pub use aws::AwsSecretsManagerSource;

#[cfg(feature = "vault")]
pub use vault::VaultSource;

#[cfg(feature = "infisical")]
pub use infisical::InfisicalSource;
