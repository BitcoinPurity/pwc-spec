//! PWC-TRANSPORT-LOCAL-001 v0.1.0 on macOS and Linux.

use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bitcoin::hashes::{sha256, Hash, HashEngine, Hmac, HmacEngine};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::error::{err, ErrorCode, PwcError};
use crate::prompt::{PairingRequest, PeerInfo};
use crate::provider::{Outbound, Provider};
use crate::types::{random_bytes, Hex, Id, Text, MAX_SAFE_INTEGER};

pub const PROTOCOL: &str = "pwc-local/1";
pub const MAX_FRAME_BYTES: usize = 1_048_576;
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

fn proof(key: &[u8; 32], label: &[u8], first: &[u8; 32], second: &[u8; 32], client_id: &str) -> [u8; 32] {
    let mut engine = HmacEngine::<sha256::Hash>::new(key);
    engine.input(label);
    engine.input(&[0]);
    engine.input(first);
    engine.input(second);
    engine.input(client_id.as_bytes());
    Hmac::<sha256::Hash>::from_engine(engine).to_byte_array()
}

/// LOCAL §5.4 `client_proof`.
pub fn client_proof(secret: &[u8; 32], server_nonce: &[u8; 32], client_nonce: &[u8; 32], client_id: &str) -> [u8; 32] {
    proof(secret, b"PWC-LOCAL-1 client", server_nonce, client_nonce, client_id)
}

/// LOCAL §5.4 `server_proof`.
pub fn server_proof(secret: &[u8; 32], server_nonce: &[u8; 32], client_nonce: &[u8; 32], client_id: &str) -> [u8; 32] {
    proof(secret, b"PWC-LOCAL-1 provider", client_nonce, server_nonce, client_id)
}

// ---------------------------------------------------------------------------------------------------------
// Endpoint (LOCAL §3)

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{bind, default_directory, serve, Endpoint};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{bind_pipe, pipe_name, serve, Endpoint};

#[derive(Debug)]
pub enum EndpointError {
    UnsafeDirectory(PathBuf),
    PathTooLong(PathBuf),
    AlreadyRunning(PathBuf),
    /// Windows: the pipe name exists and is not served by another instance under this user (LOCAL §3.3).
    NameTaken(PathBuf),
    Io(PathBuf, std::io::Error),
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeDirectory(p) => write!(
                f,
                "endpoint directory {} must belong to the current user and grant no permission to group or others",
                p.display()
            ),
            Self::PathTooLong(p) => write!(
                f,
                "endpoint path {} is longer than the operating system allows",
                p.display()
            ),
            Self::AlreadyRunning(p) => write!(f, "another Provider instance is listening on {}", p.display()),
            Self::NameTaken(p) => write!(
                f,
                "the pipe {} exists but is not served by a Provider of the current user; it is not used",
                p.display()
            ),
            Self::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for EndpointError {}

// ---------------------------------------------------------------------------------------------------------
// Frames and JSON-RPC (LOCAL §4)

enum Read {
    Frame(Vec<u8>),
    TooLong,
    Eof,
}

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Read {
    let mut buf = Vec::new();
    match (&mut *reader)
        .take(MAX_FRAME_BYTES as u64)
        .read_until(b'\n', &mut buf)
        .await
    {
        Ok(0) | Err(_) => Read::Eof,
        Ok(_) if buf.ends_with(b"\n") => Read::Frame(buf),
        Ok(n) if n == MAX_FRAME_BYTES => Read::TooLong,
        Ok(_) => Read::Eof,
    }
}

enum Parsed {
    ParseError,
    Invalid(Value),
    Notification,
    Request { id: Value, method: String, params: Value },
}

fn valid_id(id: &Value) -> bool {
    match id {
        Value::String(s) => (1..=64).contains(&s.chars().count()),
        Value::Number(n) => {
            n.as_i64().is_some_and(|v| v.unsigned_abs() <= MAX_SAFE_INTEGER)
                || n.as_u64().is_some_and(|v| v <= MAX_SAFE_INTEGER)
        }
        _ => false,
    }
}

fn parse(line: &[u8]) -> Parsed {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return Parsed::ParseError;
    };
    let Value::Object(mut o) = value else {
        return Parsed::Invalid(Value::Null);
    };
    let id = o.remove("id");
    let well_formed = o.get("jsonrpc") == Some(&json!("2.0"))
        && o.get("method").is_some_and(Value::is_string)
        && o.get("params").is_some_and(Value::is_object);
    match id {
        None if well_formed => Parsed::Notification,
        Some(id) if well_formed && valid_id(&id) => Parsed::Request {
            id,
            method: o["method"].as_str().unwrap().to_string(),
            params: o.remove("params").unwrap(),
        },
        Some(id) if valid_id(&id) => Parsed::Invalid(id),
        _ => Parsed::Invalid(Value::Null),
    }
}

fn frame(v: Value) -> Outbound {
    let mut bytes = serde_json::to_vec(&v).expect("JSON value serializes");
    bytes.push(b'\n');
    Outbound::Frame(bytes)
}

fn response(id: &Value, result: Result<Value, PwcError>) -> Outbound {
    frame(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": 1000, "message": e.message, "data": e}}),
    })
}

fn rpc_error(id: &Value, code: i32, message: &str) -> Outbound {
    frame(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
}

// ---------------------------------------------------------------------------------------------------------
// Connection lifecycle (LOCAL §5)

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HelloParams {
    protocols: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairParams {
    client_name: Text,
    client_description: Option<Text>,
    pairing_code: String,
    delegator: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticateParams {
    client_id: Id,
    client_nonce: Hex,
    client_proof: Hex,
}

fn params<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, PwcError> {
    serde_json::from_value(v).map_err(|e| err(ErrorCode::InvalidArgument, e.to_string()))
}

fn bytes32(h: Hex) -> Result<[u8; 32], PwcError> {
    h.0.try_into()
        .map_err(|_| err(ErrorCode::InvalidArgument, "expected 32 bytes"))
}

/// A pairing prompt in progress on a connection.
struct Pairing {
    id: Value,
    task: JoinHandle<Result<(), PwcError>>,
    request: PairingRequest,
}

enum Stage {
    New,
    Helloed {
        nonce: [u8; 32],
        pairing: Option<Pairing>,
        paired_once: bool,
    },
    Authenticated {
        session_id: Id,
    },
}

struct Conn {
    provider: Arc<Provider>,
    provider_id: Arc<str>,
    peer: PeerInfo,
    out: mpsc::Sender<Outbound>,
    kill: Arc<Notify>,
    stage: Stage,
    outstanding: HashSet<String>,
    connected_at: Instant,
    hello_at: Option<Instant>,
}

/// What the reader loop does after handling a frame.
enum Next {
    Continue,
    Close,
}

/// Serves one connection whose peer passed the check of LOCAL §3.4.
pub(crate) async fn connection<R, W>(provider: Arc<Provider>, provider_id: Arc<str>, rd: R, mut wr: W, peer: PeerInfo)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out, mut out_rx) = mpsc::channel::<Outbound>(provider.config().event_buffer);
    let writer = tokio::spawn(async move {
        while let Some(o) = out_rx.recv().await {
            match o {
                Outbound::Frame(b) => {
                    if wr.write_all(&b).await.is_err() {
                        break;
                    }
                }
                Outbound::Close => break,
            }
        }
        let _ = wr.shutdown().await;
    });
    let mut conn = Conn {
        provider,
        provider_id,
        peer,
        out,
        kill: Arc::new(Notify::new()),
        stage: Stage::New,
        outstanding: HashSet::new(),
        connected_at: Instant::now(),
        hello_at: None,
    };
    let mut reader = BufReader::new(rd);
    let close_with_drain = conn.run(&mut reader).await;
    if let Stage::Helloed { pairing: Some(p), .. } = &conn.stage {
        p.task.abort(); // LOCAL §5.3: withdraw the prompt; no credential is created
    }
    if let Stage::Authenticated { session_id, .. } = &conn.stage {
        conn.provider.close_session(session_id);
    }
    let _ = tokio::time::timeout(DRAIN_TIMEOUT, conn.out.send(Outbound::Close)).await;
    drop(conn);
    let abort = writer.abort_handle();
    if tokio::time::timeout(DRAIN_TIMEOUT, writer).await.is_err() {
        abort.abort(); // the Client stopped reading
    }
    if close_with_drain {
        // Discard what the Client is still sending, so that closing does not reset the connection before
        // the Client has read the last response.
        let _ = tokio::time::timeout(DRAIN_TIMEOUT, async {
            let mut sink = [0u8; 8192];
            while matches!(reader.read(&mut sink).await, Ok(n) if n > 0) {}
        })
        .await;
    }
}

impl Conn {
    fn deadline(&self) -> Option<Instant> {
        let c = self.provider.config();
        match &self.stage {
            Stage::New => Some(self.connected_at + c.hello_timeout),
            Stage::Helloed { pairing: None, .. } => Some(self.hello_at.unwrap() + c.auth_timeout),
            _ => None,
        }
    }

    /// Runs until the connection should close; returns whether the Provider initiated the close.
    async fn run<R: AsyncRead + Unpin>(&mut self, reader: &mut BufReader<R>) -> bool {
        enum Event {
            Read(Read),
            Pairing(Result<(), PwcError>),
            Stop,
        }
        loop {
            let event = {
                let deadline = self.deadline();
                let kill = self.kill.clone();
                let task = match &mut self.stage {
                    Stage::Helloed { pairing: Some(p), .. } => Some(&mut p.task),
                    _ => None,
                };
                let pairing = async move {
                    match task {
                        Some(t) => t
                            .await
                            .unwrap_or_else(|_| Err(err(ErrorCode::InternalError, "pairing task failed"))),
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    r = read_frame(reader) => Event::Read(r),
                    d = pairing => Event::Pairing(d),
                    _ = kill.notified() => Event::Stop,
                    _ = async { tokio::time::sleep_until(deadline.unwrap()).await }, if deadline.is_some() => Event::Stop,
                }
            };
            match event {
                Event::Read(Read::Eof) => return false,
                Event::Read(Read::TooLong) => {
                    let e = err(ErrorCode::PayloadTooLarge, "Frame exceeds max_frame_bytes");
                    self.send(response(&Value::Null, Err(e))).await;
                    return true;
                }
                Event::Read(Read::Frame(line)) => {
                    if let Next::Close = self.on_frame(&line).await {
                        return true;
                    }
                }
                Event::Pairing(d) => self.on_pairing_decision(d).await,
                Event::Stop => return true,
            }
        }
    }

    async fn send(&self, o: Outbound) {
        let _ = self.out.send(o).await;
    }

    async fn on_frame(&mut self, line: &[u8]) -> Next {
        let (id, method, params) = match parse(line) {
            Parsed::ParseError => {
                self.send(rpc_error(&Value::Null, -32700, "Parse error")).await;
                return Next::Continue;
            }
            Parsed::Invalid(id) => {
                self.send(rpc_error(&id, -32600, "Invalid Request")).await;
                return Next::Continue;
            }
            Parsed::Notification => return Next::Continue,
            Parsed::Request { id, method, params } => (id, method, params),
        };
        let key = id.to_string();
        if self.outstanding.contains(&key) {
            self.send(rpc_error(&id, -32600, "Invalid Request")).await;
            return Next::Continue;
        }
        if method == "local.pair"
            && matches!(
                self.stage,
                Stage::Helloed {
                    pairing: None,
                    paired_once: false,
                    ..
                }
            )
        {
            return self.start_pairing(id, key, params).await;
        }
        let (result, next) = self.evaluate(&method, params).await;
        self.send(response(&id, result)).await;
        next
    }

    async fn evaluate(&mut self, method: &str, params: Value) -> (Result<Value, PwcError>, Next) {
        let unauthenticated = |m: &str| (Err(err(ErrorCode::Unauthenticated, m)), Next::Continue);
        let unsupported = |m: &str| (Err(err(ErrorCode::UnsupportedOperation, m)), Next::Continue);
        match &self.stage {
            Stage::New => {
                if method != "local.hello" {
                    return (
                        Err(err(ErrorCode::Unauthenticated, "local.hello must come first")),
                        Next::Close,
                    );
                }
                self.hello(params)
            }
            Stage::Helloed { pairing, nonce, .. } => match method {
                "local.hello" => unsupported("local.hello was already received"),
                "local.pair" => (
                    Err(err(ErrorCode::RateLimited, "Only one pairing prompt per connection")),
                    Next::Continue,
                ),
                "local.authenticate" if pairing.is_some() => unsupported("Pairing is in progress"),
                "local.authenticate" => {
                    let nonce = *nonce;
                    self.authenticate(params, nonce).await
                }
                _ => unauthenticated("Not authenticated"),
            },
            Stage::Authenticated { session_id, .. } => match method {
                "local.unpair" => {
                    if let Err(e) = self::params::<EmptyParams>(params) {
                        return (Err(e), Next::Continue);
                    }
                    self.provider.unpair_session(session_id);
                    (Ok(json!({"unpaired": true})), Next::Continue)
                }
                "local.hello" | "local.pair" | "local.authenticate" => {
                    unsupported("The connection is already authenticated")
                }
                "local.openDelegate" | "local.delegated" | "local.closeDelegate" => {
                    unsupported("Delegated principals are not implemented")
                }
                _ if method.starts_with("local.") => (
                    Err(err(
                        ErrorCode::UnsupportedMethod,
                        format!("Method {method} is not implemented"),
                    )),
                    Next::Continue,
                ),
                _ => {
                    // On the blocking pool: the request may wait for the backend lock or a backend call.
                    let (provider, session_id, method) =
                        (self.provider.clone(), session_id.clone(), method.to_string());
                    let outcome = tokio::task::spawn_blocking(move || provider.handle(&session_id, &method, params))
                        .await
                        .unwrap_or_else(|_| Err(err(ErrorCode::InternalError, "The request failed")));
                    (outcome, Next::Continue)
                }
            },
        }
    }

    fn hello(&mut self, params: Value) -> (Result<Value, PwcError>, Next) {
        let p: HelloParams = match self::params(params) {
            Ok(p) => p,
            Err(e) => return (Err(e), Next::Continue),
        };
        if !p.protocols.iter().any(|x| x == PROTOCOL) {
            return (
                Err(err(ErrorCode::UnsupportedOperation, "No common protocol")),
                Next::Close,
            );
        }
        let nonce = random_bytes::<32>();
        self.stage = Stage::Helloed {
            nonce,
            pairing: None,
            paired_once: false,
        };
        self.hello_at = Some(Instant::now());
        let c = self.provider.config();
        let result = json!({
            "protocol": PROTOCOL,
            "pwc_version": "0.2.1",
            "provider_id": &*self.provider_id,
            "provider": {"name": c.provider_name, "version": c.provider_version},
            "max_frame_bytes": MAX_FRAME_BYTES,
            "server_nonce": Hex(nonce.to_vec()),
            "delegation": false,
        });
        (Ok(result), Next::Continue)
    }

    async fn start_pairing(&mut self, id: Value, key: String, params: Value) -> Next {
        let p: PairParams = match self::params(params) {
            Ok(p) => p,
            Err(e) => {
                self.send(response(&id, Err(e))).await;
                return Next::Continue;
            }
        };
        if p.pairing_code.len() != 6 || !p.pairing_code.bytes().all(|b| b.is_ascii_digit()) {
            self.send(response(
                &id,
                Err(err(ErrorCode::InvalidArgument, "pairing_code must be 6 decimal digits")),
            ))
            .await;
            return Next::Continue;
        }
        if p.delegator == Some(true) {
            let e = err(
                ErrorCode::UnsupportedOperation,
                "Delegated principals are not implemented",
            );
            self.send(response(&id, Err(e))).await;
            return Next::Continue;
        }
        let request = PairingRequest {
            claimed_name: p.client_name,
            claimed_description: p.client_description,
            pairing_code: p.pairing_code,
            peer: self.peer.clone(),
        };
        let provider = self.provider.clone();
        let prompt = request.clone();
        let task = tokio::spawn(async move { provider.pairing_prompt(prompt).await });
        self.outstanding.insert(key);
        if let Stage::Helloed {
            pairing, paired_once, ..
        } = &mut self.stage
        {
            *pairing = Some(Pairing { id, task, request });
            *paired_once = true;
        }
        Next::Continue
    }

    async fn on_pairing_decision(&mut self, decision: Result<(), PwcError>) {
        let Stage::Helloed { pairing, .. } = &mut self.stage else {
            return;
        };
        let Pairing { id, request, .. } = pairing.take().expect("pairing in progress");
        self.outstanding.remove(&id.to_string());
        let result = match decision {
            Ok(()) => {
                let (client_id, secret) = self.provider.register_client(&request);
                let session_id = self
                    .provider
                    .open_session(&client_id, self.out.clone(), self.kill.clone());
                let r =
                    json!({"client_id": client_id, "client_secret": Hex(secret.to_vec()), "session_id": session_id});
                self.stage = Stage::Authenticated { session_id };
                Ok(r)
            }
            Err(e) => Err(e),
        };
        self.send(response(&id, result)).await;
    }

    async fn authenticate(&mut self, params: Value, server_nonce: [u8; 32]) -> (Result<Value, PwcError>, Next) {
        let p: AuthenticateParams = match self::params(params) {
            Ok(p) => p,
            Err(e) => return (Err(e), Next::Close),
        };
        let (nonce, proof) = match (bytes32(p.client_nonce), bytes32(p.client_proof)) {
            (Ok(n), Ok(pr)) => (n, pr),
            (Err(e), _) | (_, Err(e)) => return (Err(e), Next::Close),
        };
        let pid = self.peer.pid.unwrap_or(-1);
        match self
            .provider
            .authenticate(pid, p.client_id.as_str(), &server_nonce, &nonce, &proof)
        {
            Ok(server) => {
                let session_id = self
                    .provider
                    .open_session(&p.client_id, self.out.clone(), self.kill.clone());
                let r = json!({"session_id": session_id, "server_proof": Hex(server.to_vec())});
                self.stage = Stage::Authenticated { session_id };
                (Ok(r), Next::Continue)
            }
            Err(e) => {
                if e.code == ErrorCode::Unauthenticated {
                    tokio::time::sleep(self.provider.config().auth_failure_delay).await;
                }
                (Err(e), Next::Close)
            }
        }
    }
}
