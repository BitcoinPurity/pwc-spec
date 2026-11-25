//! The Client side of the LOCAL transport v0.1.0: endpoint directory, discovery, frames, JSON-RPC 2.0 and the
//! handshake.

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;

use crate::client::Client;
use crate::credential::Credential;
use crate::error::{Error, PwcError};
use crate::types::{HelloInfo, PairParams, PwcEvent};

pub const PROTOCOL: &str = "pwc-local/1";
/// The smallest `max_frame_bytes` a Provider may announce (LOCAL §4.1); the limit before `local.hello` answers.
const MIN_MAX_FRAME_BYTES: u64 = 1_048_576;
const EVENT_BUFFER: usize = 256;

/// A Provider's socket in an endpoint directory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Endpoint {
    pub provider_id: String,
    pub path: PathBuf,
}

/// LOCAL §3.1: `[a-z0-9-]{1,32}`.
pub(crate) fn valid_provider_id(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn euid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

fn resolve_directory(xdg: Option<OsString>, home: Option<OsString>) -> Result<PathBuf, Error> {
    let home = || {
        home.map(PathBuf::from)
            .ok_or_else(|| Error::UnsafeEndpoint("HOME is not set".into()))
    };
    if cfg!(target_os = "macos") {
        return Ok(home()?.join("Library/Application Support/PWC/run"));
    }
    if let Some(xdg) = xdg.map(PathBuf::from) {
        if xdg.is_absolute() && std::fs::metadata(&xdg).is_ok_and(|m| m.uid() == euid()) {
            return Ok(xdg.join("pwc"));
        }
    }
    Ok(home()?.join(".pwc/run"))
}

/// The endpoint directory of LOCAL §3.2 for the current user.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn endpoint_directory() -> Result<PathBuf, Error> {
    resolve_directory(std::env::var_os("XDG_RUNTIME_DIR"), std::env::var_os("HOME"))
}

/// LOCAL §3.2: the directory must belong to the user and grant nothing to group or others.
fn check_directory(dir: &Path) -> Result<(), Error> {
    let unsafe_dir = |why: &str| Error::UnsafeEndpoint(format!("{}: {why}", dir.display()));
    let meta = std::fs::metadata(dir).map_err(|e| unsafe_dir(&e.to_string()))?;
    if !meta.is_dir() {
        return Err(unsafe_dir("not a directory"));
    }
    if meta.uid() != euid() {
        return Err(unsafe_dir("owned by another user"));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(unsafe_dir("accessible to group or others"));
    }
    Ok(())
}

/// Lists the Providers with a socket in `dir` (LOCAL §3.5), sorted by `provider_id`. A directory that does not
/// exist holds none. Let the User choose if there are several.
pub fn discover(dir: &Path) -> Result<Vec<Endpoint>, Error> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    check_directory(dir)?;
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().is_some_and(|e| e == "sock") && valid_provider_id(stem) {
            found.push(Endpoint {
                provider_id: stem.to_string(),
                path,
            });
        }
    }
    found.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    Ok(found)
}

fn proof(label: &[u8], secret: &[u8; 32], first: &[u8; 32], second: &[u8; 32], client_id: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(label);
    mac.update(&[0]);
    mac.update(first);
    mac.update(second);
    mac.update(client_id.as_bytes());
    mac
}

/// LOCAL §5.4: `HMAC-SHA256(K, "PWC-LOCAL-1 client" ‖ 0x00 ‖ SN ‖ CN ‖ ID)`.
pub fn client_proof(secret: &[u8; 32], server_nonce: &[u8; 32], client_nonce: &[u8; 32], client_id: &str) -> [u8; 32] {
    proof(b"PWC-LOCAL-1 client", secret, server_nonce, client_nonce, client_id)
        .finalize()
        .into_bytes()
        .into()
}

/// LOCAL §5.4: `HMAC-SHA256(K, "PWC-LOCAL-1 provider" ‖ 0x00 ‖ CN ‖ SN ‖ ID)`.
pub fn server_proof(secret: &[u8; 32], server_nonce: &[u8; 32], client_nonce: &[u8; 32], client_id: &str) -> [u8; 32] {
    proof(b"PWC-LOCAL-1 provider", secret, client_nonce, server_nonce, client_id)
        .finalize()
        .into_bytes()
        .into()
}

pub(crate) fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("the operating system provides randomness");
    b
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// Six random decimal digits for `local.pair` (LOCAL §5.3). Show the code to the User, who compares it with
/// the one in the wallet's prompt.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn pairing_code() -> String {
    loop {
        let v = u32::from_le_bytes(random());
        if v < 4_294_000_000 {
            return format!("{:06}", v % 1_000_000);
        }
    }
}

type Waiting = HashMap<u64, oneshot::Sender<Result<Value, Error>>>;

/// What the reader task shares with the callers. Both members are `None` once the connection is closed.
struct Shared {
    waiting: Mutex<Option<Waiting>>,
    events: Mutex<Option<broadcast::Sender<PwcEvent>>>,
    max_frame_bytes: AtomicU64,
}

impl Shared {
    fn close(&self) {
        self.waiting.lock().unwrap().take();
        self.events.lock().unwrap().take();
    }
}

/// One line without its terminator; `None` at end of stream, on an error, or past `limit` bytes.
async fn read_frame(reader: &mut BufReader<OwnedReadHalf>, limit: u64) -> Option<Vec<u8>> {
    let mut frame = Vec::new();
    loop {
        let chunk = reader.fill_buf().await.ok()?;
        if chunk.is_empty() {
            return None;
        }
        if let Some(i) = chunk.iter().position(|b| *b == b'\n') {
            frame.extend_from_slice(&chunk[..i]);
            reader.consume(i + 1);
            return Some(frame);
        }
        let n = chunk.len();
        frame.extend_from_slice(chunk);
        reader.consume(n);
        if frame.len() as u64 > limit {
            return None;
        }
    }
}

/// LOCAL §4.3.
fn rpc_error(error: &Value) -> Error {
    if error["code"] == 1000 {
        return match PwcError::deserialize(&error["data"]) {
            Ok(e) => Error::Pwc(e),
            Err(e) => Error::Protocol(format!("error data: {e}")),
        };
    }
    Error::Rpc {
        code: error["code"].as_i64().unwrap_or(0),
        message: error["message"].as_str().unwrap_or("").to_string(),
    }
}

async fn read_loop(read: OwnedReadHalf, shared: Arc<Shared>) {
    let mut reader = BufReader::new(read);
    while let Some(line) = read_frame(&mut reader, shared.max_frame_bytes.load(Ordering::Relaxed)).await {
        let Ok(frame) = serde_json::from_slice::<Value>(&line) else {
            break;
        };
        if let Some(id) = frame.get("id").and_then(Value::as_u64) {
            let waiter = shared.waiting.lock().unwrap().as_mut().and_then(|w| w.remove(&id));
            if let Some(waiter) = waiter {
                let _ = waiter.send(match frame.get("result") {
                    Some(r) => Ok(r.clone()),
                    None => Err(rpc_error(&frame["error"])),
                });
            }
        } else if frame["method"] == "pwc.event" {
            if let Ok(event) = PwcEvent::deserialize(&frame["params"]) {
                if let Some(events) = shared.events.lock().unwrap().as_ref() {
                    let _ = events.send(event);
                }
            }
        }
    }
    shared.close();
}

/// A connected socket with its reader task. Dropping it closes the socket.
pub(crate) struct Link {
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    shared: Arc<Shared>,
    next_id: AtomicU64,
    reader: JoinHandle<()>,
}

impl Drop for Link {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl Link {
    fn new(stream: UnixStream) -> Link {
        let (read, write) = stream.into_split();
        let shared = Arc::new(Shared {
            waiting: Mutex::new(Some(HashMap::new())),
            events: Mutex::new(Some(broadcast::channel(EVENT_BUFFER).0)),
            max_frame_bytes: AtomicU64::new(MIN_MAX_FRAME_BYTES),
        });
        Link {
            writer: tokio::sync::Mutex::new(write),
            reader: tokio::spawn(read_loop(read, shared.clone())),
            shared,
            next_id: AtomicU64::new(1),
        }
    }

    pub(crate) async fn request(&self, method: &str, params: Value) -> Result<Value, Error> {
        if !params.is_object() {
            return Err(Error::Protocol("params must be an object".into()));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut frame = serde_json::to_vec(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .expect("a JSON value serializes");
        frame.push(b'\n');
        if frame.len() as u64 > self.shared.max_frame_bytes.load(Ordering::Relaxed) {
            return Err(Error::FrameTooLarge);
        }
        let (tx, rx) = oneshot::channel();
        match self.shared.waiting.lock().unwrap().as_mut() {
            Some(waiting) => waiting.insert(id, tx),
            None => return Err(Error::ConnectionClosed),
        };
        if self.writer.lock().await.write_all(&frame).await.is_err() {
            self.shared.close();
            return Err(Error::ConnectionClosed);
        }
        rx.await.unwrap_or(Err(Error::ConnectionClosed))
    }

    /// A receiver of the events that arrive from now on; already closed if the connection is.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<PwcEvent> {
        match self.shared.events.lock().unwrap().as_ref() {
            Some(events) => events.subscribe(),
            None => broadcast::channel(1).1,
        }
    }

    pub(crate) async fn close(&self) {
        self.shared.close();
        self.reader.abort();
        let _ = self.writer.lock().await.shutdown().await;
    }
}

#[derive(Deserialize)]
struct Hello {
    #[serde(flatten)]
    info: HelloInfo,
    server_nonce: String,
}

#[derive(Deserialize)]
struct Paired {
    client_id: String,
    client_secret: String,
    session_id: String,
}

#[derive(Deserialize)]
struct Authenticated {
    session_id: String,
    server_proof: String,
}

fn parse<T: serde::de::DeserializeOwned>(method: &str, value: Value) -> Result<T, Error> {
    serde_json::from_value(value).map_err(|e| Error::Protocol(format!("result of {method}: {e}")))
}

/// A connection on which `local.hello` has been answered. It becomes a `Client` by `pair` or `authenticate`.
pub struct Connection {
    link: Link,
    hello: HelloInfo,
    server_nonce: [u8; 32],
}

impl Connection {
    /// Checks the endpoint directory, connects to the socket at `path` and sends `local.hello`.
    pub async fn open(path: &Path) -> Result<Connection, Error> {
        check_directory(path.parent().unwrap_or(Path::new("/")))?;
        let link = Link::new(UnixStream::connect(path).await?);
        let hello: Hello = parse(
            "local.hello",
            link.request("local.hello", json!({"protocols": [PROTOCOL]})).await?,
        )?;
        if hello.info.protocol != PROTOCOL {
            return Err(Error::Protocol(format!(
                "unexpected protocol {:?}",
                hello.info.protocol
            )));
        }
        if path.file_stem().and_then(|s| s.to_str()) != Some(hello.info.provider_id.as_str()) {
            return Err(Error::Protocol("provider_id does not match the socket name".into()));
        }
        let server_nonce =
            unhex32(&hello.server_nonce).ok_or_else(|| Error::Protocol("malformed server_nonce".into()))?;
        link.shared
            .max_frame_bytes
            .store(hello.info.max_frame_bytes, Ordering::Relaxed);
        Ok(Connection {
            link,
            hello: hello.info,
            server_nonce,
        })
    }

    /// What the Provider said about itself. It is unauthenticated until `authenticate` succeeds.
    pub fn hello(&self) -> HelloInfo {
        self.hello.clone()
    }

    /// Pairs with the Provider (LOCAL §5.3). Returns when the User decides in the wallet: `USER_REJECTED` if
    /// refused, `OPERATION_EXPIRED` if unanswered. Store the credential where other users cannot read it.
    pub async fn pair(self, params: PairParams) -> Result<(Client, Credential), Error> {
        let params = serde_json::to_value(params).expect("pairing parameters serialize");
        let paired: Paired = parse("local.pair", self.link.request("local.pair", params).await?)?;
        if unhex32(&paired.client_secret).is_none() {
            return Err(Error::Protocol("malformed client_secret".into()));
        }
        let credential = Credential {
            provider_id: self.hello.provider_id.clone(),
            client_id: paired.client_id.clone(),
            client_secret: paired.client_secret,
        };
        Ok((
            Client::new(self.link, self.hello, paired.client_id, paired.session_id),
            credential,
        ))
    }

    /// Authenticates both sides (LOCAL §5.4). If the Provider's proof is wrong, the connection is closed
    /// without another byte being sent, and the result is `ProviderAuthFailed`.
    pub async fn authenticate(self, credential: &Credential) -> Result<Client, Error> {
        let secret =
            unhex32(&credential.client_secret).ok_or_else(|| Error::Protocol("malformed client_secret".into()))?;
        let client_nonce: [u8; 32] = random();
        let id = &credential.client_id;
        let params = json!({
            "client_id": id,
            "client_nonce": hex(&client_nonce),
            "client_proof": hex(&client_proof(&secret, &self.server_nonce, &client_nonce, id)),
        });
        let result: Authenticated = parse(
            "local.authenticate",
            self.link.request("local.authenticate", params).await?,
        )?;
        let expected = proof(b"PWC-LOCAL-1 provider", &secret, &client_nonce, &self.server_nonce, id);
        // `verify_slice` compares in constant time.
        let genuine = unhex32(&result.server_proof).is_some_and(|p| expected.verify_slice(&p).is_ok());
        if !genuine {
            return Err(Error::ProviderAuthFailed);
        }
        Ok(Client::new(self.link, self.hello, id.clone(), result.session_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_follows_local_3_2() {
        let home = Some(OsString::from("/home/u"));
        if cfg!(target_os = "macos") {
            let dir = resolve_directory(None, home).unwrap();
            assert_eq!(dir, Path::new("/home/u/Library/Application Support/PWC/run"));
            return;
        }
        let own = tempfile::tempdir().unwrap();
        let xdg = Some(own.path().as_os_str().to_owned());
        assert_eq!(resolve_directory(xdg, home.clone()).unwrap(), own.path().join("pwc"));
        assert_eq!(
            resolve_directory(None, home.clone()).unwrap(),
            Path::new("/home/u/.pwc/run")
        );
        // Relative, or missing and so not owned by the user: fall back to the home directory.
        for bad in ["relative/run", "/nonexistent-pwc-runtime-dir"] {
            let dir = resolve_directory(Some(bad.into()), home.clone()).unwrap();
            assert_eq!(dir, Path::new("/home/u/.pwc/run"));
        }
        assert!(matches!(resolve_directory(None, None), Err(Error::UnsafeEndpoint(_))));
    }

    #[test]
    fn pairing_code_is_six_digits() {
        for _ in 0..100 {
            let code = pairing_code();
            assert!(code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()), "{code}");
        }
    }
}
