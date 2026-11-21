//! Transport-independent Provider: Core semantics, state and approval flow.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Notify};

use crate::error::{err, ErrorCode, PwcError};
use crate::prompt::{PairingRequest, PeerInfo, Prompt};
use crate::types::{random_bytes, Id, Text, Timestamp};

pub type PromptReceiver = mpsc::UnboundedReceiver<Prompt>;

#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub max: usize,
    pub window: Duration,
}

/// Timeouts and limits. `Default` gives the values of `docs/api-contract.md`.
#[derive(Debug, Clone)]
pub struct Config {
    pub provider_name: String,
    pub provider_version: String,
    pub hello_timeout: Duration,
    pub auth_timeout: Duration,
    pub pairing_timeout: Duration,
    pub pairing_limit: RateLimit,
    pub auth_failure_limit: RateLimit,
    pub auth_failure_delay: Duration,
    pub event_buffer: usize,
}

impl Default for Config {
    fn default() -> Self {
        let minute = Duration::from_secs(60);
        Config {
            provider_name: "PWC Provider".into(),
            provider_version: env!("CARGO_PKG_VERSION").into(),
            hello_timeout: Duration::from_secs(10),
            auth_timeout: Duration::from_secs(60),
            pairing_timeout: Duration::from_secs(300),
            pairing_limit: RateLimit { max: 10, window: minute },
            auth_failure_limit: RateLimit { max: 10, window: minute },
            auth_failure_delay: Duration::from_millis(100),
            event_buffer: 1024,
        }
    }
}

/// A paired Client as shown in the Provider's list of paired Clients (LOCAL §5.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedClient {
    pub client_id: Id,
    pub claimed_name: Text,
    pub claimed_description: Option<Text>,
    pub paired_at: Timestamp,
    pub last_connected_at: Timestamp,
    pub peer: PeerInfo,
}

#[derive(Serialize, Deserialize)]
struct ClientRecord {
    info: PairedClient,
    #[serde(with = "hex32")]
    secret: [u8; 32],
}

mod hex32 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&bitcoin::hex::DisplayHex::to_lower_hex_string(&v[..]))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        bitcoin::hex::FromHex::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// State that survives in a snapshot.
#[derive(Default, Serialize, Deserialize)]
struct State {
    clients: BTreeMap<Id, ClientRecord>,
}

/// Frames queued for one connection's writer.
pub(crate) enum Outbound {
    Frame(Vec<u8>),
    Close,
}

/// A live authenticated connection.
pub(crate) struct SessionHandle {
    pub client_id: Id,
    pub created_at: Timestamp,
    pub out: mpsc::Sender<Outbound>,
    pub kill: Arc<Notify>,
}

struct RateLog<K>(HashMap<K, VecDeque<Instant>>);

impl<K: Hash + Eq> RateLog<K> {
    fn new() -> Self {
        RateLog(HashMap::new())
    }

    fn full(&mut self, key: &K, limit: RateLimit) -> bool {
        let now = Instant::now();
        match self.0.get_mut(key) {
            Some(log) => {
                while log.front().is_some_and(|t| now.duration_since(*t) >= limit.window) {
                    log.pop_front();
                }
                log.len() >= limit.max
            }
            None => false,
        }
    }

    fn record(&mut self, key: K) {
        self.0.entry(key).or_default().push_back(Instant::now());
    }
}

struct Inner {
    state: State,
    sessions: HashMap<Id, SessionHandle>,
    pairings: RateLog<i32>,
    auth_failures: RateLog<i32>,
}

pub struct Provider {
    config: Config,
    prompts: mpsc::UnboundedSender<Prompt>,
    inner: Mutex<Inner>,
}

const AUTH_FAILED: &str = "Authentication failed";

impl Provider {
    pub fn new(config: Config) -> (Arc<Provider>, PromptReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = Inner {
            state: State::default(),
            sessions: HashMap::new(),
            pairings: RateLog::new(),
            auth_failures: RateLog::new(),
        };
        (Arc::new(Provider { config, prompts: tx, inner: Mutex::new(inner) }), rx)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// The Provider's list of paired Clients.
    pub fn paired_clients(&self) -> Vec<PairedClient> {
        self.lock().state.clients.values().map(|c| c.info.clone()).collect()
    }

    /// Shows a pairing prompt and waits for the User (LOCAL §5.3). Dropping the returned future withdraws
    /// the prompt.
    pub(crate) async fn pairing_prompt(&self, request: PairingRequest) -> Result<(), PwcError> {
        let pid = request.peer.pid.unwrap_or(-1);
        {
            let mut inner = self.lock();
            if inner.pairings.full(&pid, self.config.pairing_limit) {
                return Err(err(ErrorCode::RateLimited, "Too many pairing requests"));
            }
            inner.pairings.record(pid);
        }
        let (reply, answer) = oneshot::channel();
        if self.prompts.send(Prompt::Pairing { request, reply }).is_err() {
            return Err(err(ErrorCode::UserRejected, "Pairing refused"));
        }
        match tokio::time::timeout(self.config.pairing_timeout, answer).await {
            Ok(Ok(true)) => Ok(()),
            Ok(_) => Err(err(ErrorCode::UserRejected, "Pairing refused")),
            Err(_) => Err(err(ErrorCode::OperationExpired, "Pairing prompt was not answered in time")),
        }
    }

    /// Creates the credential of a newly paired Client.
    pub(crate) fn register_client(&self, request: &PairingRequest) -> (Id, [u8; 32]) {
        let client_id = Id::random("cl");
        let secret = random_bytes::<32>();
        let now = Timestamp::now();
        let info = PairedClient {
            client_id: client_id.clone(),
            claimed_name: request.claimed_name.clone(),
            claimed_description: request.claimed_description.clone(),
            paired_at: now,
            last_connected_at: now,
            peer: request.peer.clone(),
        };
        self.lock().state.clients.insert(client_id.clone(), ClientRecord { info, secret });
        (client_id, secret)
    }

    /// Verifies a `local.authenticate` proof and returns the server proof (LOCAL §5.4).
    pub(crate) fn authenticate(
        &self,
        pid: i32,
        client_id: &str,
        server_nonce: &[u8; 32],
        client_nonce: &[u8; 32],
        client_proof: &[u8; 32],
    ) -> Result<[u8; 32], PwcError> {
        let mut inner = self.lock();
        if inner.auth_failures.full(&pid, self.config.auth_failure_limit) {
            return Err(err(ErrorCode::RateLimited, "Too many failed authentication attempts"));
        }
        let known = inner.state.clients.get_mut(client_id);
        let secret = known.as_ref().map(|c| c.secret).unwrap_or([0; 32]);
        let expected = crate::local::client_proof(&secret, server_nonce, client_nonce, client_id);
        let proof_ok = bitcoin::hashes::cmp::fixed_time_eq(&expected, client_proof);
        match known {
            Some(c) if proof_ok => {
                c.info.last_connected_at = Timestamp::now();
                Ok(crate::local::server_proof(&secret, server_nonce, client_nonce, client_id))
            }
            _ => {
                inner.auth_failures.record(pid);
                Err(err(ErrorCode::Unauthenticated, AUTH_FAILED))
            }
        }
    }

    pub(crate) fn open_session(&self, client_id: &Id, out: mpsc::Sender<Outbound>, kill: Arc<Notify>) -> Id {
        let session_id = Id::random("ss");
        let handle = SessionHandle { client_id: client_id.clone(), created_at: Timestamp::now(), out, kill };
        self.lock().sessions.insert(session_id.clone(), handle);
        session_id
    }

    pub(crate) fn close_session(&self, session_id: &Id) {
        self.lock().sessions.remove(session_id);
    }

    /// Executes a Core method for an authenticated session.
    pub(crate) fn handle(self: &Arc<Self>, _session_id: &Id, method: &str, _params: Value) -> Result<Value, PwcError> {
        Err(err(ErrorCode::UnsupportedMethod, format!("Method {method} is not implemented")))
    }
}
