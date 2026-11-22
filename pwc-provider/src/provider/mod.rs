//! Transport-independent Provider: Core semantics, state and approval flow.

mod grants;
mod handlers;
mod operations;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Notify};

use crate::backend::{AccountInfo, BackendEvent, Reserved, WalletBackend, WalletInfo, WalletState};
use crate::error::{err, ErrorCode, PwcError};
use crate::methods::{self, Resource};
use crate::prompt::{PairingRequest, PeerInfo, Prompt, Requester};
use crate::types::{random_bytes, FeeRate, Id, Text, Timestamp};

pub use grants::Grant;
use operations::{IdemRecord, OpRecord, Replay, Reservation};
pub use operations::{OpState, Operation};

pub type PromptReceiver = mpsc::UnboundedReceiver<Prompt>;

#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub max: usize,
    pub window: Duration,
}

/// `Capabilities.limits` (Core §4.1).
#[derive(Debug, Clone, Serialize)]
pub struct Limits {
    pub max_page_size: u64,
    pub max_psbt_bytes: u64,
    pub max_combine_psbts: u64,
    pub max_raw_transaction_bytes: u64,
    pub max_message_bytes: u64,
    pub max_concurrent_operations: u64,
    pub operation_retention_seconds: u64,
    pub high_fee_rate: FeeRate,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_page_size: 100,
            max_psbt_bytes: 100_000,
            max_combine_psbts: 5,
            max_raw_transaction_bytes: 400_000,
            max_message_bytes: 65_536,
            max_concurrent_operations: 8,
            operation_retention_seconds: 86_400,
            high_fee_rate: FeeRate(500_000),
        }
    }
}

/// Timeouts and limits. `Default` gives the values of `docs/api-contract.md`.
#[derive(Debug, Clone)]
pub struct Config {
    pub provider_name: String,
    pub provider_version: String,
    pub limits: Limits,
    pub hello_timeout: Duration,
    pub auth_timeout: Duration,
    pub pairing_timeout: Duration,
    pub approval_timeout: Duration,
    pub reservation_ttl: Duration,
    pub pairing_limit: RateLimit,
    pub auth_failure_limit: RateLimit,
    pub auth_failure_delay: Duration,
    pub prompt_limit: RateLimit,
    pub event_buffer: usize,
}

impl Default for Config {
    fn default() -> Self {
        let minute = Duration::from_secs(60);
        Config {
            provider_name: "PWC Provider".into(),
            provider_version: env!("CARGO_PKG_VERSION").into(),
            limits: Limits::default(),
            hello_timeout: Duration::from_secs(10),
            auth_timeout: Duration::from_secs(60),
            pairing_timeout: Duration::from_secs(300),
            approval_timeout: Duration::from_secs(300),
            reservation_ttl: Duration::from_secs(600),
            pairing_limit: RateLimit {
                max: 10,
                window: minute,
            },
            auth_failure_limit: RateLimit {
                max: 10,
                window: minute,
            },
            auth_failure_delay: Duration::from_millis(100),
            prompt_limit: RateLimit {
                max: 10,
                window: minute,
            },
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
#[derive(Serialize, Deserialize)]
struct State {
    #[serde(with = "hex32")]
    cursor_key: [u8; 32],
    clients: BTreeMap<Id, ClientRecord>,
    grants: BTreeMap<Id, Grant>,
    operations: BTreeMap<Id, OpRecord>,
    idempotency: BTreeMap<String, IdemRecord>,
    reservations: BTreeMap<Id, Reservation>,
}

const SNAPSHOT_VERSION: u64 = 1;

#[derive(Serialize)]
struct SnapshotRef<'a> {
    version: u64,
    #[serde(flatten)]
    state: &'a State,
}

#[derive(Deserialize)]
struct Snapshot {
    #[serde(flatten)]
    state: State,
}

#[derive(Debug)]
pub enum StateError {
    Corrupt(String),
    UnsupportedVersion(u64),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt(m) => write!(f, "corrupt Provider state: {m}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported Provider state version {v}"),
        }
    }
}

impl std::error::Error for StateError {}

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
    prompt_log: RateLog<Id>,
    /// Dropping an entry stops the task waiting on that Operation's prompt, which withdraws the prompt.
    pending: HashMap<Id, oneshot::Sender<()>>,
}

pub struct Provider {
    config: Config,
    backend: Arc<dyn WalletBackend>,
    prompts: mpsc::UnboundedSender<Prompt>,
    inner: Mutex<Inner>,
}

const AUTH_FAILED: &str = "Authentication failed";

/// Who may receive an event.
enum Audience<'a> {
    Client(&'a Id),
    Wallet {
        wallet: &'a WalletInfo,
        readable_only: bool,
    },
    Account {
        account: &'a AccountInfo,
        scope: &'static str,
    },
}

/// A request's resource after check 7.
struct Target {
    wallet: WalletInfo,
    account: Option<AccountInfo>,
}

/// The result of check 10.
enum Outcome {
    Value(Value),
    /// A synchronous result containing the account data of the target wallet.
    AccountData(Value),
    Operation(Id),
}

impl Provider {
    pub fn new(backend: Arc<dyn WalletBackend>, config: Config) -> (Arc<Provider>, PromptReceiver) {
        let state = State {
            cursor_key: random_bytes(),
            clients: BTreeMap::new(),
            grants: BTreeMap::new(),
            operations: BTreeMap::new(),
            idempotency: BTreeMap::new(),
            reservations: BTreeMap::new(),
        };
        Self::from_state(backend, config, state)
    }

    fn from_state(backend: Arc<dyn WalletBackend>, config: Config, state: State) -> (Arc<Provider>, PromptReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = Inner {
            state,
            sessions: HashMap::new(),
            pairings: RateLog::new(),
            auth_failures: RateLog::new(),
            prompt_log: RateLog::new(),
            pending: HashMap::new(),
        };
        (
            Arc::new(Provider {
                config,
                backend,
                prompts: tx,
                inner: Mutex::new(inner),
            }),
            rx,
        )
    }

    /// Restores a Provider from a snapshot made by `save_state`. Operations that were waiting for the User
    /// fail with a retryable `INTERNAL_ERROR`, because their prompts did not survive.
    pub fn with_state(
        backend: Arc<dyn WalletBackend>,
        config: Config,
        snapshot: &[u8],
    ) -> Result<(Arc<Provider>, PromptReceiver), StateError> {
        let value: Value = serde_json::from_slice(snapshot).map_err(|e| StateError::Corrupt(e.to_string()))?;
        match value.get("version").and_then(Value::as_u64) {
            Some(SNAPSHOT_VERSION) => {}
            Some(v) => return Err(StateError::UnsupportedVersion(v)),
            None => return Err(StateError::Corrupt("no version".into())),
        }
        let Snapshot { mut state } = serde_json::from_value(value).map_err(|e| StateError::Corrupt(e.to_string()))?;
        let now = Timestamp::now();
        for rec in state.operations.values_mut().filter(|o| !o.op.state.terminal()) {
            let mut e = err(
                ErrorCode::InternalError,
                "The Provider restarted while the Operation was waiting",
            );
            e.retryable = Some(true);
            rec.op.state = OpState::Failed;
            rec.op.updated_at = now;
            rec.op.error = Some(e);
            rec.finished_at = Some(now);
        }
        let grants: Vec<Grant> = state.grants.values().cloned().collect();
        let (provider, prompts) = Self::from_state(backend, config, state);
        provider.housekeeping(&mut provider.lock(), now);
        if tokio::runtime::Handle::try_current().is_ok() {
            grants.iter().for_each(|g| provider.schedule_expiry(g));
        }
        Ok((provider, prompts))
    }

    /// A snapshot of the paired Clients and their secrets, grants, Operations, idempotency records and
    /// reservations. It contains secrets: the host stores it at least as safely as its wallet files (LOCAL §7).
    pub fn save_state(&self) -> Vec<u8> {
        let inner = self.lock();
        serde_json::to_vec(&SnapshotRef {
            version: SNAPSHOT_VERSION,
            state: &inner.state,
        })
        .expect("state serializes")
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    // -----------------------------------------------------------------------------------------------------
    // Host-facing API

    /// The Provider's list of paired Clients.
    pub fn paired_clients(&self) -> Vec<PairedClient> {
        self.lock().state.clients.values().map(|c| c.info.clone()).collect()
    }

    /// Every grant, as `Grant` JSON objects, for the wallet's grant review UI (Core §6).
    pub fn grants(&self) -> Vec<Value> {
        let now = Timestamp::now();
        self.lock().state.grants.values().map(|g| g.to_json(now)).collect()
    }

    /// Revokes a grant from the wallet's own interface.
    pub fn revoke_grant(&self, grant_id: &str) -> bool {
        let mut inner = self.lock();
        self.revoke(&mut inner, grant_id)
    }

    /// Unpairs a Client from the wallet's own interface; same effect as `local.unpair` (LOCAL §5.7).
    pub fn unpair_client(&self, client_id: &str) {
        let mut inner = self.lock();
        self.unpair(&mut inner, client_id);
    }

    // -----------------------------------------------------------------------------------------------------
    // Transport-facing API

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
            Err(_) => Err(err(
                ErrorCode::OperationExpired,
                "Pairing prompt was not answered in time",
            )),
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
        self.lock()
            .state
            .clients
            .insert(client_id.clone(), ClientRecord { info, secret });
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
                Ok(crate::local::server_proof(
                    &secret,
                    server_nonce,
                    client_nonce,
                    client_id,
                ))
            }
            _ => {
                inner.auth_failures.record(pid);
                Err(err(ErrorCode::Unauthenticated, AUTH_FAILED))
            }
        }
    }

    pub(crate) fn open_session(&self, client_id: &Id, out: mpsc::Sender<Outbound>, kill: Arc<Notify>) -> Id {
        let session_id = Id::random("ss");
        let handle = SessionHandle {
            client_id: client_id.clone(),
            created_at: Timestamp::now(),
            out,
            kill,
        };
        self.lock().sessions.insert(session_id.clone(), handle);
        session_id
    }

    pub(crate) fn close_session(&self, session_id: &Id) {
        self.lock().sessions.remove(session_id);
    }

    /// `local.unpair` for the Client of this session.
    pub(crate) fn unpair_session(&self, session_id: &Id) {
        let mut inner = self.lock();
        if let Some(client_id) = inner.sessions.get(session_id).map(|s| s.client_id.clone()) {
            self.unpair(&mut inner, client_id.as_str());
        }
    }

    /// Executes a Core method for an authenticated session in the order of checks of Core §8.
    pub(crate) fn handle(self: &Arc<Self>, session_id: &Id, method: &str, params: Value) -> Result<Value, PwcError> {
        // 3: method implemented
        let spec = methods::lookup(method).ok_or_else(|| {
            err(
                ErrorCode::UnsupportedMethod,
                format!("Method {method} is not implemented"),
            )
        })?;
        // 4: parameter syntax and size limits
        let request = methods::parse(spec, params.clone(), &self.config.limits)?;
        let mut guard = self.lock();
        let inner = &mut *guard;
        let client_id = match inner.sessions.get(session_id) {
            Some(s) => s.client_id.clone(),
            None => return Err(err(ErrorCode::Unauthenticated, "Session closed")),
        };
        let now = Timestamp::now();
        self.housekeeping(inner, now);
        // 5: idempotency
        let idem_key = spec.idempotent.then(|| {
            let key = params["idempotency_key"].as_str().unwrap_or_default();
            format!("{client_id}\u{0}{method}\u{0}{key}")
        });
        if let Some(record) = idem_key.as_ref().and_then(|k| inner.state.idempotency.get(k)) {
            if record.params != params {
                return Err(err(
                    ErrorCode::IdempotencyConflict,
                    "Idempotency key reused with different parameters",
                ));
            }
            return self.replay(inner, &record.replay.clone(), now);
        }
        // 6: scope
        if let Some(scope) = spec.scope {
            self.check_scope(inner, &client_id, scope, now)?;
        }
        // 7: resource
        let target = match spec.resource {
            Resource::None => None,
            Resource::Wallet => Some(self.covered_wallet(inner, &client_id, spec.scope.unwrap(), &params, now)?),
            Resource::Account => Some(self.covered_account(inner, &client_id, spec.scope.unwrap(), &params, now)?),
        };
        // 8: wallet state
        if let Some(t) = &target {
            if spec.reads_account_data {
                wallet_readable(&t.wallet)?;
            }
        }
        // 9: concurrency and prompt rate
        if spec.operation {
            let open = inner
                .state
                .operations
                .values()
                .filter(|o| o.client_id == client_id && !o.op.state.terminal())
                .count();
            if open as u64 >= self.config.limits.max_concurrent_operations {
                return Err(err(ErrorCode::RateLimited, "Too many unfinished Operations"));
            }
        }
        if spec.prompts && inner.prompt_log.full(&client_id, self.config.prompt_limit) {
            return Err(err(ErrorCode::RateLimited, "Too many approval requests"));
        }
        // 10: method-specific checks and execution
        let ctx = handlers::Ctx {
            client_id: &client_id,
            session_id,
            now,
            target: target.as_ref(),
        };
        let outcome = self.execute(inner, &ctx, request)?;
        let (response, replay) = match outcome {
            Outcome::Value(v) => (
                v.clone(),
                Replay::Value {
                    result: v,
                    wallet_id: None,
                },
            ),
            Outcome::AccountData(v) => {
                let wallet_id = target.as_ref().map(|t| t.wallet.wallet_id.clone());
                (v.clone(), Replay::Value { result: v, wallet_id })
            }
            Outcome::Operation(id) => (self.view(inner, &id, now), Replay::Operation(id)),
        };
        if let Some(key) = idem_key {
            inner.state.idempotency.insert(
                key,
                IdemRecord {
                    params,
                    replay,
                    created_at: now,
                },
            );
        }
        Ok(response)
    }

    // -----------------------------------------------------------------------------------------------------
    // Checks

    fn active_grants<'a>(
        &self,
        inner: &'a Inner,
        client_id: &'a Id,
        now: Timestamp,
    ) -> impl Iterator<Item = &'a Grant> + 'a {
        inner
            .state
            .grants
            .values()
            .filter(move |g| &g.client_id == client_id && g.active(now))
    }

    fn check_scope(&self, inner: &Inner, client_id: &Id, scope: &str, now: Timestamp) -> Result<(), PwcError> {
        if self.active_grants(inner, client_id, now).any(|g| g.has_scope(scope)) {
            return Ok(());
        }
        let held_before = inner
            .state
            .grants
            .values()
            .any(|g| &g.client_id == client_id && g.has_scope(scope));
        if held_before {
            Err(err(
                ErrorCode::PermissionRevoked,
                format!("Scope {scope} is no longer granted"),
            ))
        } else {
            Err(err(ErrorCode::PermissionRequired, format!("Scope {scope} is required")))
        }
    }

    fn covered_wallet(
        &self,
        inner: &Inner,
        client_id: &Id,
        scope: &str,
        params: &Value,
        now: Timestamp,
    ) -> Result<Target, PwcError> {
        let not_found = || err(ErrorCode::WalletNotFound, "Wallet not found");
        let wallet = self
            .backend
            .wallet(params["wallet_id"].as_str().unwrap_or_default())
            .ok_or_else(not_found)?;
        if self
            .active_grants(inner, client_id, now)
            .any(|g| g.has_scope(scope) && g.covers_wallet(&wallet))
        {
            Ok(Target { wallet, account: None })
        } else {
            Err(not_found())
        }
    }

    fn covered_account(
        &self,
        inner: &Inner,
        client_id: &Id,
        scope: &str,
        params: &Value,
        now: Timestamp,
    ) -> Result<Target, PwcError> {
        let not_found = || err(ErrorCode::AccountNotFound, "Account not found");
        let account = self
            .backend
            .account(params["account_id"].as_str().unwrap_or_default())
            .ok_or_else(not_found)?;
        if !self
            .active_grants(inner, client_id, now)
            .any(|g| g.has_scope(scope) && g.covers_account(&account))
        {
            return Err(not_found());
        }
        let wallet = self.backend.wallet(account.wallet_id.as_str()).ok_or_else(not_found)?;
        Ok(Target {
            wallet,
            account: Some(account),
        })
    }

    /// Whether the Client currently holds `scope` for the Operation's account (or at all, without one).
    fn authorized(&self, inner: &Inner, client_id: &Id, scope: &str, account_id: Option<&Id>, now: Timestamp) -> bool {
        let account = account_id.and_then(|a| self.backend.account(a.as_str()));
        if account_id.is_some() && account.is_none() {
            return false;
        }
        self.active_grants(inner, client_id, now)
            .any(|g| g.has_scope(scope) && account.as_ref().is_none_or(|a| g.covers_account(a)))
    }

    fn replay(&self, inner: &Inner, replay: &Replay, now: Timestamp) -> Result<Value, PwcError> {
        match replay {
            Replay::Operation(id) => Ok(self.view(inner, id, now)),
            Replay::Value { result, wallet_id } => {
                if let Some(w) = wallet_id.as_ref().and_then(|w| self.backend.wallet(w.as_str())) {
                    wallet_readable(&w)?;
                }
                Ok(result.clone())
            }
        }
    }

    /// The Operation as its Client may see it now (Core §8.5).
    fn view(&self, inner: &Inner, op_id: &Id, now: Timestamp) -> Value {
        let rec = &inner.state.operations[op_id];
        let mut op = rec.op.clone();
        if op.result.is_some() && op.method != "session.requestPermissions" {
            let scope_held = rec
                .scope
                .as_deref()
                .is_none_or(|s| self.authorized(inner, &rec.client_id, s, op.account_id.as_ref(), now));
            let wallet_ok = rec
                .wallet_id
                .as_ref()
                .and_then(|w| self.backend.wallet(w.as_str()))
                .is_none_or(|w| wallet_readable(&w).is_ok());
            if !scope_held || !wallet_ok {
                op.result = None;
            }
        }
        serde_json::to_value(op).expect("Operation serializes")
    }

    // -----------------------------------------------------------------------------------------------------
    // Operations

    fn new_operation(&self, inner: &mut Inner, ctx: &handlers::Ctx, method: &str, state: OpState) -> Id {
        let operation_id = Id::random("op");
        let spec = methods::lookup(method).expect("registered method");
        let account = ctx.target.and_then(|t| t.account.as_ref());
        let op = Operation {
            operation_id: operation_id.clone(),
            method: method.to_string(),
            state,
            created_at: ctx.now,
            updated_at: ctx.now,
            expires_at: (!state.terminal()).then(|| ctx.now.after(self.config.approval_timeout)),
            account_id: account.map(|a| a.account_id.clone()),
            result: None,
            error: None,
        };
        let rec = OpRecord {
            op,
            client_id: ctx.client_id.clone(),
            scope: spec.scope.map(str::to_string),
            wallet_id: ctx.target.map(|t| t.wallet.wallet_id.clone()),
            finished_at: None,
        };
        inner.state.operations.insert(operation_id.clone(), rec);
        operation_id
    }

    /// Moves a nonterminal Operation to `state`. Terminal Operations never change (Core §9.1).
    fn transition(
        &self,
        inner: &mut Inner,
        op_id: &Id,
        state: OpState,
        result: Option<Value>,
        error: Option<PwcError>,
    ) {
        let now = Timestamp::now();
        let Some(rec) = inner.state.operations.get_mut(op_id) else {
            return;
        };
        if rec.op.state.terminal() {
            return;
        }
        rec.op.state = state;
        rec.op.updated_at = now;
        rec.op.result = result;
        rec.op.error = error;
        if state.terminal() {
            rec.finished_at = Some(now);
            inner.pending.remove(op_id);
        }
        let client_id = rec.client_id.clone();
        self.emit(
            inner,
            "operation.stateChanged",
            None,
            Some(op_id),
            json!({ "state": state }),
            Audience::Client(&client_id),
        );
    }

    fn fail(&self, inner: &mut Inner, op_id: &Id, e: PwcError) {
        self.transition(inner, op_id, OpState::Failed, None, Some(e));
    }

    /// Sends `prompt` and waits on its answer in a task. `decide` runs under the lock with the answer if the
    /// Operation is still awaiting approval; refusal, expiry and cancellation are handled here.
    fn await_approval<T, F>(
        self: &Arc<Self>,
        inner: &mut Inner,
        op_id: &Id,
        client_id: &Id,
        prompt: Prompt,
        answer: oneshot::Receiver<T>,
        decide: F,
    ) where
        T: Send + 'static,
        F: FnOnce(&Arc<Provider>, &mut Inner, &Id, T) + Send + 'static,
    {
        inner.prompt_log.record(client_id.clone());
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        inner.pending.insert(op_id.clone(), cancel_tx);
        let _ = self.prompts.send(prompt);
        let expires_at = inner.state.operations[op_id]
            .op
            .expires_at
            .expect("awaiting Operations expire");
        let provider = self.clone();
        let op_id = op_id.clone();
        tokio::spawn(async move {
            tokio::select! {
                a = answer => {
                    let mut guard = provider.lock();
                    let inner = &mut *guard;
                    let awaiting = inner.state.operations.get(&op_id).is_some_and(|o| o.op.state == OpState::AwaitingApproval);
                    match a {
                        Ok(v) if awaiting => decide(&provider, inner, &op_id, v),
                        _ => {
                            let e = err(ErrorCode::UserRejected, "The User refused the request");
                            provider.transition(inner, &op_id, OpState::Rejected, None, Some(e));
                        }
                    }
                }
                _ = tokio::time::sleep(expires_at.remaining()) => {
                    let mut inner = provider.lock();
                    provider.expire_operation(&mut inner, &op_id);
                }
                _ = cancel_rx => {}
            }
        });
    }

    fn expire_operation(&self, inner: &mut Inner, op_id: &Id) {
        let e = err(ErrorCode::OperationExpired, "The approval window elapsed");
        self.transition(inner, op_id, OpState::Expired, None, Some(e));
    }

    /// Lazy expiry of Operations and pruning of records past their retention (Core §8.5, §9.3).
    fn housekeeping(&self, inner: &mut Inner, now: Timestamp) {
        let due: Vec<Id> = inner
            .state
            .operations
            .values()
            .filter(|o| !o.op.state.terminal() && o.op.expires_at.is_some_and(|e| e <= now))
            .map(|o| o.op.operation_id.clone())
            .collect();
        for id in due {
            self.expire_operation(inner, &id);
        }
        let retention = Duration::from_secs(self.config.limits.operation_retention_seconds);
        inner
            .state
            .operations
            .retain(|_, o| o.finished_at.is_none_or(|f| f.after(retention) > now));
        inner.state.reservations.retain(|_, r| r.until.after(retention) > now);
        let ops = &inner.state.operations;
        inner.state.idempotency.retain(|_, r| match &r.replay {
            Replay::Operation(id) => ops.contains_key(id),
            Replay::Value { .. } => r.created_at.after(retention) > now,
        });
    }

    // -----------------------------------------------------------------------------------------------------
    // Revocation

    fn revoke(&self, inner: &mut Inner, grant_id: &str) -> bool {
        let now = Timestamp::now();
        let Some(g) = inner.state.grants.get(grant_id) else {
            return false;
        };
        if !g.active(now) {
            return false;
        }
        let client_id = g.client_id.clone();
        self.emit(
            inner,
            "grant.revoked",
            None,
            None,
            json!({ "grant_id": grant_id }),
            Audience::Client(&client_id),
        );
        inner.state.grants.get_mut(grant_id).expect("found above").revoked = true;
        self.reevaluate(inner, &client_id);
        true
    }

    /// Core §7 step 6: unfinished Operations that are no longer authorized fail with `PERMISSION_REVOKED`.
    fn reevaluate(&self, inner: &mut Inner, client_id: &Id) {
        let now = Timestamp::now();
        let stale: Vec<Id> = inner
            .state
            .operations
            .values()
            .filter(|o| &o.client_id == client_id && !o.op.state.terminal())
            .filter(|o| {
                o.scope
                    .as_deref()
                    .is_some_and(|s| !self.authorized(inner, client_id, s, o.op.account_id.as_ref(), now))
            })
            .map(|o| o.op.operation_id.clone())
            .collect();
        for id in stale {
            self.fail(inner, &id, err(ErrorCode::PermissionRevoked, "The grant was revoked"));
        }
    }

    fn unpair(&self, inner: &mut Inner, client_id: &str) {
        let Some(client) = inner.state.clients.remove(client_id) else {
            return;
        };
        let client_id = client.info.client_id;
        let now = Timestamp::now();
        let active: Vec<Id> = inner
            .state
            .grants
            .values()
            .filter(|g| g.client_id == client_id && g.active(now))
            .map(|g| g.grant_id.clone())
            .collect();
        for id in &active {
            self.emit(
                inner,
                "grant.revoked",
                None,
                None,
                json!({ "grant_id": id }),
                Audience::Client(&client_id),
            );
        }
        for g in inner.state.grants.values_mut().filter(|g| g.client_id == client_id) {
            g.revoked = true;
        }
        let open: Vec<Id> = inner
            .state
            .operations
            .values()
            .filter(|o| o.client_id == client_id && !o.op.state.terminal())
            .map(|o| o.op.operation_id.clone())
            .collect();
        for id in open {
            self.fail(inner, &id, err(ErrorCode::PermissionRevoked, "The Client was unpaired"));
        }
        for s in inner.sessions.values().filter(|s| s.client_id == client_id) {
            s.kill.notify_one();
        }
    }

    /// A reservation ends when it expires, is released, its grant ends or one of its inputs is spent (Core §8.3).
    fn reservation_active(&self, inner: &Inner, r: &Reservation, now: Timestamp) -> bool {
        !r.released
            && now < r.until
            && inner.state.grants.get(&r.grant_id).is_some_and(|g| g.active(now))
            && r.outpoints
                .iter()
                .all(|o| self.backend.is_unspent(r.account_id.as_str(), o))
    }

    /// Outpoints of the account under an active reservation of any Client.
    fn reserved(&self, inner: &Inner, account_id: &Id, now: Timestamp) -> Reserved {
        inner
            .state
            .reservations
            .values()
            .filter(|r| &r.account_id == account_id && self.reservation_active(inner, r, now))
            .flat_map(|r| r.outpoints.iter().copied())
            .collect()
    }

    /// An Operation that is already terminal when created, for methods that complete synchronously.
    fn finished_operation(
        &self,
        inner: &mut Inner,
        ctx: &handlers::Ctx,
        method: &str,
        outcome: Result<Value, PwcError>,
    ) -> Id {
        let id = self.new_operation(inner, ctx, method, OpState::Completed);
        let rec = inner.state.operations.get_mut(&id).expect("just created");
        match outcome {
            Ok(r) => rec.op.result = Some(r),
            Err(e) => {
                rec.op.state = OpState::Failed;
                rec.op.error = Some(e);
            }
        }
        rec.finished_at = Some(ctx.now);
        id
    }

    /// Reports a wallet change; authorized Clients receive the corresponding event.
    pub fn notify(&self, event: BackendEvent) {
        let inner = self.lock();
        match &event {
            BackendEvent::WalletLocked { wallet_id } | BackendEvent::WalletUnlocked { wallet_id } => {
                let Some(wallet) = self.backend.wallet(wallet_id.as_str()) else {
                    return;
                };
                let kind = if matches!(event, BackendEvent::WalletLocked { .. }) {
                    "wallet.locked"
                } else {
                    "wallet.unlocked"
                };
                let audience = Audience::Wallet {
                    wallet: &wallet,
                    readable_only: false,
                };
                self.emit(&inner, kind, None, None, json!({ "wallet_id": wallet_id }), audience);
            }
            BackendEvent::WalletSyncChanged { wallet_id } => {
                let Some(wallet) = self.backend.wallet(wallet_id.as_str()) else {
                    return;
                };
                let state = if wallet.state == WalletState::Syncing {
                    "syncing"
                } else {
                    "synced"
                };
                let payload = json!({ "wallet_id": wallet_id, "sync": { "state": state } });
                self.emit(
                    &inner,
                    "wallet.syncChanged",
                    None,
                    None,
                    payload,
                    Audience::Wallet {
                        wallet: &wallet,
                        readable_only: true,
                    },
                );
            }
            BackendEvent::BalanceChanged { account_id } => {
                let Some(account) = self.backend.account(account_id.as_str()) else {
                    return;
                };
                let audience = Audience::Account {
                    account: &account,
                    scope: "balances:read",
                };
                self.emit(
                    &inner,
                    "account.balanceChanged",
                    Some(account_id),
                    None,
                    json!({}),
                    audience,
                );
            }
            BackendEvent::TransactionChanged {
                account_id,
                txid,
                status,
            } => {
                let Some(account) = self.backend.account(account_id.as_str()) else {
                    return;
                };
                let audience = Audience::Account {
                    account: &account,
                    scope: "transactions:read",
                };
                let payload = json!({ "txid": txid, "status": status });
                self.emit(
                    &inner,
                    "account.transactionChanged",
                    Some(account_id),
                    None,
                    payload,
                    audience,
                );
            }
        }
    }

    /// Delivers an event to every connection of a Client that holds `events:read` and could perform the
    /// equivalent read (Core §8.6, LOCAL §8). A connection whose output buffer is full is closed.
    fn emit(
        &self,
        inner: &Inner,
        event_type: &str,
        account_id: Option<&Id>,
        operation_id: Option<&Id>,
        payload: Value,
        audience: Audience,
    ) {
        let now = Timestamp::now();
        let mut event =
            json!({ "event_id": Id::random("ev"), "event_type": event_type, "occurred_at": now, "payload": payload });
        if let Some(a) = account_id {
            event["account_id"] = json!(a);
        }
        if let Some(o) = operation_id {
            event["operation_id"] = json!(o);
        }
        let mut frame = serde_json::to_vec(&json!({ "jsonrpc": "2.0", "method": "pwc.event", "params": event }))
            .expect("event serializes");
        frame.push(b'\n');
        let readable = |w: &WalletInfo| wallet_readable(w).is_ok();
        let wallet_of = |a: &AccountInfo| self.backend.wallet(a.wallet_id.as_str());
        for s in inner.sessions.values() {
            let grants: Vec<&Grant> = self.active_grants(inner, &s.client_id, now).collect();
            if !grants.iter().any(|g| g.has_scope("events:read")) {
                continue;
            }
            let allowed = match &audience {
                Audience::Client(c) => &s.client_id == *c,
                Audience::Wallet { wallet, readable_only } => {
                    (!readable_only || readable(wallet))
                        && grants
                            .iter()
                            .any(|g| g.has_scope("wallets:read") && g.covers_wallet(wallet))
                }
                Audience::Account { account, scope } => {
                    wallet_of(account).is_some_and(|w| readable(&w))
                        && grants.iter().any(|g| g.has_scope(scope) && g.covers_account(account))
                }
            };
            if allowed && s.out.try_send(Outbound::Frame(frame.clone())).is_err() {
                s.kill.notify_one();
            }
        }
    }

    fn requester(&self, inner: &Inner, client_id: &Id) -> Requester {
        let info = &inner.state.clients[client_id].info;
        Requester {
            client_id: client_id.clone(),
            claimed_name: info.claimed_name.clone(),
            peer: info.peer.clone(),
        }
    }

    fn capabilities(&self) -> Value {
        json!({
            "pwc_version": "0.2.1",
            "provider": {"name": self.config.provider_name, "version": self.config.provider_version, "platform": "desktop"},
            "profiles": ["pwc-core-read-v1", "pwc-psbt-signer-v1"],
            "supported_networks": crate::types::SUPPORTED_NETWORKS,
            "methods": methods::METHODS.iter().map(|m| m.name).collect::<Vec<_>>(),
            "features": ["coin_control", "watch_only", "event_push"],
            "scopes": crate::types::SCOPES,
            "script_types": ["p2wpkh", "p2pkh"],
            "message_formats": ["bip322-simple", "legacy"],
            "limits": self.config.limits,
        })
    }
}

/// Check 8 for methods that read account data (Core §8).
fn wallet_readable(w: &WalletInfo) -> Result<(), PwcError> {
    match w.state {
        WalletState::Locked => Err(err(ErrorCode::WalletLocked, "The wallet is locked")),
        WalletState::Unavailable => Err(err(ErrorCode::StateUnavailable, "The wallet is unavailable")),
        WalletState::Ready | WalletState::Syncing => Ok(()),
    }
}
