//! PWC Client: PWC-CORE-001 v0.2.1 over PWC-TRANSPORT-LOCAL-001 v0.1.0 (macOS and Linux).
//!
//! ```no_run
//! # async fn example() -> Result<(), pwc_client::Error> {
//! use std::sync::Arc;
//! use pwc_client::*;
//!
//! let endpoint = discover(&endpoint_directory()?)?.remove(0);
//! let store = Arc::new(FileCredentialStore::new("/home/me/.config/myapp/pwc"));
//! let client = match Client::connect(&endpoint, store.clone()).await {
//!     Err(Error::NotPaired) => {
//!         let code = pairing_code();
//!         println!("Compare this code with the wallet: {code}");
//!         let params = PairParams { client_name: "My App".into(), client_description: None, pairing_code: code };
//!         Client::pair(&endpoint, store, params).await?
//!     }
//!     other => other?,
//! };
//! let op = client
//!     .request_permissions(RequestPermissionsParams {
//!         scopes: vec!["wallets:read".into()],
//!         network: None,
//!         wallet_ids: None,
//!         account_ids: None,
//!         expires_at: None,
//!         constraints: None,
//!         idempotency_key: idempotency_key(),
//!     })
//!     .await?;
//! let op = client.wait_operation(&op.operation_id, std::time::Duration::from_secs(300)).await?;
//! let grants = op.permissions_result()?;
//! # Ok(()) }
//! ```

pub mod client;
pub mod credential;
pub mod error;
#[cfg(feature = "uniffi")]
mod ffi;
pub mod local;
pub mod types;

pub use client::{idempotency_key, Client, EventStream};
pub use credential::{Credential, CredentialStore, FileCredentialStore};
pub use error::{Error, ErrorCode, PwcError};
pub use local::{discover, endpoint_directory, pairing_code, Connection, Endpoint};
pub use types::*;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
