use std::fmt;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::local::valid_provider_id;

/// What a Client keeps after pairing (LOCAL §5.3). `client_secret` is 64 hexadecimal characters; never log it,
/// pass it on a command line or put it in an environment variable (LOCAL §7).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Credential {
    pub provider_id: String,
    pub client_id: String,
    pub client_secret: String,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Credential")
            .field("provider_id", &self.provider_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

/// Where the application keeps its credentials, one per Provider. Implement it over the operating system's
/// credential store where there is one (LOCAL §7).
pub trait CredentialStore: Send + Sync {
    fn load(&self, provider_id: &str) -> Result<Option<Credential>, Error>;
    /// Replaces any credential stored for `credential.provider_id`.
    fn save(&self, credential: &Credential) -> Result<(), Error>;
    fn delete(&self, provider_id: &str) -> Result<(), Error>;
}

/// Keeps each credential in `<dir>/<provider_id>.json`, readable by the current user only.
pub struct FileCredentialStore {
    dir: PathBuf,
}

impl FileCredentialStore {
    pub fn new(dir: impl Into<PathBuf>) -> FileCredentialStore {
        FileCredentialStore { dir: dir.into() }
    }

    fn file(&self, provider_id: &str) -> Result<PathBuf, Error> {
        if !valid_provider_id(provider_id) {
            return Err(Error::Protocol(format!("invalid provider_id {provider_id:?}")));
        }
        Ok(self.dir.join(format!("{provider_id}.json")))
    }
}

impl CredentialStore for FileCredentialStore {
    fn load(&self, provider_id: &str) -> Result<Option<Credential>, Error> {
        match fs::read(self.file(provider_id)?) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| Error::Io(format!("credential file: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, credential: &Credential) -> Result<(), Error> {
        let path = self.file(&credential.provider_id)?;
        fs::DirBuilder::new().recursive(true).mode(0o700).create(&self.dir)?;
        self.delete(&credential.provider_id)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(&serde_json::to_vec(credential).expect("a credential serializes"))?;
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), Error> {
        match fs::remove_file(self.file(provider_id)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}
