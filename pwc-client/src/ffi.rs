//! Feature `uniffi`: what a binding needs beyond the annotations on the public interface.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::{Client, Credential, Endpoint, Error, EventStream, FileCredentialStore, PairParams};

// JSON crosses the boundary as text; `uniffi.toml` turns it into the language's own objects.
uniffi::custom_type!(Value, String, {
    remote,
    lower: |value| value.to_string(),
    try_lift: |text| Ok(serde_json::from_str(&text)?),
});

uniffi::custom_type!(PathBuf, String, {
    remote,
    lower: |path| path.to_string_lossy().into_owned(),
    try_lift: |text| Ok(PathBuf::from(text)),
});

/// An exception from a foreign `CredentialStore` that is not an `Error`.
impl From<uniffi::UnexpectedUniFFICallbackError> for Error {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Error {
        Error::Io(e.reason)
    }
}

/// Lists the Providers with a socket in `dir`; see `crate::discover`.
#[uniffi::export]
pub fn discover(dir: String) -> Result<Vec<Endpoint>, Error> {
    crate::discover(Path::new(&dir))
}

/// `crate::CredentialStore` for implementations in another language, which take owned values.
#[uniffi::export(with_foreign)]
pub trait CredentialStore: Send + Sync {
    fn load(&self, provider_id: String) -> Result<Option<Credential>, Error>;
    /// Replaces any credential stored for `credential.provider_id`.
    fn save(&self, credential: Credential) -> Result<(), Error>;
    fn delete(&self, provider_id: String) -> Result<(), Error>;
}

struct Foreign(Arc<dyn CredentialStore>);

impl crate::CredentialStore for Foreign {
    fn load(&self, provider_id: &str) -> Result<Option<Credential>, Error> {
        self.0.load(provider_id.to_string())
    }

    fn save(&self, credential: &Credential) -> Result<(), Error> {
        self.0.save(credential.clone())
    }

    fn delete(&self, provider_id: &str) -> Result<(), Error> {
        self.0.delete(provider_id.to_string())
    }
}

impl CredentialStore for FileCredentialStore {
    fn load(&self, provider_id: String) -> Result<Option<Credential>, Error> {
        crate::CredentialStore::load(self, &provider_id)
    }

    fn save(&self, credential: Credential) -> Result<(), Error> {
        crate::CredentialStore::save(self, &credential)
    }

    fn delete(&self, provider_id: String) -> Result<(), Error> {
        crate::CredentialStore::delete(self, &provider_id)
    }
}

/// A store that keeps each credential in `<dir>/<provider_id>.json`, readable by the current user only.
#[uniffi::export]
pub fn file_credential_store(dir: String) -> Arc<dyn CredentialStore> {
    Arc::new(FileCredentialStore::new(dir))
}

#[uniffi::export(async_runtime = "tokio")]
impl Client {
    /// See `Client::connect`.
    #[uniffi::constructor(name = "connect")]
    pub async fn ffi_connect(endpoint: Endpoint, store: Arc<dyn CredentialStore>) -> Result<Client, Error> {
        Client::connect(&endpoint, Arc::new(Foreign(store))).await
    }

    /// See `Client::pair`.
    #[uniffi::constructor(name = "pair")]
    pub async fn ffi_pair(
        endpoint: Endpoint,
        store: Arc<dyn CredentialStore>,
        params: PairParams,
    ) -> Result<Client, Error> {
        Client::pair(&endpoint, Arc::new(Foreign(store)), params).await
    }

    /// See `Client::subscribe_events`.
    #[uniffi::method(name = "subscribe_events")]
    pub fn ffi_subscribe_events(&self) -> Arc<EventStream> {
        Arc::new(self.subscribe_events())
    }
}
