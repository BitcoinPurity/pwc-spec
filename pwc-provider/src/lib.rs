//! PWC Provider: PWC-CORE-001 v0.2.1 over PWC-TRANSPORT-LOCAL-001 v0.1.0.

pub mod backend;
pub mod error;
pub mod local;
pub mod memory;
pub mod methods;
pub mod types;
pub mod prompt;
pub mod provider;

/// LOCAL §3.1 provider identifier.
pub const PROVIDER_ID: &str = "pwc-provider";
