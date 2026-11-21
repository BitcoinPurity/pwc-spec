#![allow(dead_code)]

pub mod schema;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use pwc_provider::local;
use pwc_provider::memory::MemoryBackend;
use pwc_provider::prompt::{GrantChoice, PermissionRequest, Prompt};
use pwc_provider::types::Network;
use pwc_provider::provider::{Config, PromptReceiver, Provider};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::task::JoinHandle;

pub const WAIT: Duration = Duration::from_secs(5);

pub struct Harness {
    pub dir: tempfile::TempDir,
    pub path: PathBuf,
    pub provider: Arc<Provider>,
    pub prompts: PromptReceiver,
    pub backend: Arc<MemoryBackend>,
    server: JoinHandle<()>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Harness {
    pub async fn start(config: Config) -> Harness {
        Self::with_backend(Arc::new(MemoryBackend::demo()), config).await
    }

    pub async fn with_backend(backend: Arc<MemoryBackend>, config: Config) -> Harness {
        let (provider, prompts) = Provider::new(backend.clone(), config);
        let mut h = Self::serve(provider, prompts).await;
        h.backend = backend;
        h
    }

    pub async fn serve(provider: Arc<Provider>, prompts: PromptReceiver) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = local::bind(&dir.path().join("run"), pwc_provider::PROVIDER_ID).unwrap();
        let path = endpoint.path().to_path_buf();
        let server = tokio::spawn(local::serve(provider.clone(), endpoint));
        Harness { dir, path, provider, prompts, backend: Arc::new(MemoryBackend::demo()), server }
    }

    pub async fn next_prompt(&mut self) -> Prompt {
        tokio::time::timeout(WAIT, self.prompts.recv()).await.expect("a prompt").expect("prompt channel open")
    }

    pub async fn no_prompt(&mut self) {
        assert!(tokio::time::timeout(Duration::from_millis(200), self.prompts.recv()).await.is_err(), "unexpected prompt");
    }

    pub async fn connect(&self) -> Conn {
        Conn::connect(&self.path).await
    }

    /// Pairs a new Client, approving the pairing prompt.
    pub async fn pair(&mut self) -> (Conn, String, [u8; 32]) {
        let mut c = self.connect().await;
        c.hello().await;
        let id = c.send_request("local.pair", json!({"client_name": "Test client", "pairing_code": "123456"})).await;
        match self.next_prompt().await {
            Prompt::Pairing { reply, .. } => reply.send(true).unwrap(),
            _ => panic!("expected a pairing prompt"),
        }
        let r = c.response(id).await;
        let r = r["result"].clone();
        let secret = bitcoin::hex::FromHex::from_hex(r["client_secret"].as_str().unwrap()).unwrap();
        (c, r["client_id"].as_str().unwrap().to_string(), secret)
    }

    /// Opens a connection and authenticates a paired Client.
    pub async fn login(&self, client_id: &str, secret: &[u8; 32]) -> Conn {
        let mut c = self.connect().await;
        let h = c.hello().await;
        let r = c.authenticate(&h, client_id, secret).await;
        assert!(r.get("result").is_some(), "authentication failed: {r}");
        c
    }
}

pub struct Conn {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
    next_id: u64,
    pub events: Vec<Value>,
}

fn check_frame(v: &Value) {
    schema::local("ProviderFrame", v).unwrap();
    if v["method"] == "pwc.event" {
        schema::core("PwcEvent", &v["params"]).unwrap();
    }
}

fn check_result(method: &str, result: &Value) {
    if method.starts_with("local.") {
        schema::local(&format!("{method}.result"), result).unwrap();
        return;
    }
    if schema::core_defines(&format!("{method}.result")) {
        schema::core(&format!("{method}.result"), result).unwrap();
        if result["state"] == "completed" && result.get("result").is_some() {
            let op_method = result["method"].as_str().unwrap();
            schema::core(&format!("{op_method}.operationResult"), &result["result"]).unwrap();
        }
    }
}

impl Conn {
    pub async fn connect(path: &std::path::Path) -> Conn {
        let stream = UnixStream::connect(path).await.unwrap();
        let (r, w) = stream.into_split();
        Conn { reader: BufReader::new(r), writer: w, next_id: 1, events: Vec::new() }
    }

    pub async fn send_raw(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes).await;
    }

    pub async fn send(&mut self, v: &Value) {
        let mut b = serde_json::to_vec(v).unwrap();
        b.push(b'\n');
        self.send_raw(&b).await;
    }

    /// Reads one frame; `None` at end of stream.
    pub async fn recv(&mut self) -> Option<Value> {
        let mut line = Vec::new();
        let n = tokio::time::timeout(WAIT, self.reader.read_until(b'\n', &mut line)).await.expect("a frame in time");
        match n {
            Ok(0) | Err(_) => None,
            Ok(_) => {
                let v: Value = serde_json::from_slice(&line).unwrap();
                check_frame(&v);
                Some(v)
            }
        }
    }

    /// True if the Provider closes the connection without sending another frame.
    pub async fn closed(&mut self) -> bool {
        self.recv().await.is_none()
    }

    pub async fn send_request(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).await;
        id
    }

    /// Waits for the response with `id`, keeping event notifications in `events`.
    pub async fn response(&mut self, id: u64) -> Value {
        loop {
            let v = self.recv().await.expect("connection open");
            if v["method"] == "pwc.event" {
                self.events.push(v["params"].clone());
                continue;
            }
            assert_eq!(v["id"], json!(id), "unexpected frame {v}");
            return v;
        }
    }

    /// Sends a request and returns the raw response frame.
    pub async fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params).await;
        let v = self.response(id).await;
        if let Some(r) = v.get("result") {
            check_result(method, r);
        } else {
            let e = &v["error"];
            assert_eq!(e["code"], 1000, "{v}");
            assert_eq!(e["message"], e["data"]["message"]);
        }
        v
    }

    /// Sends a request; `Ok(result)` or `Err(PwcError)`.
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        let v = self.call(method, params).await;
        match v.get("result") {
            Some(r) => Ok(r.clone()),
            None => Err(v["error"]["data"].clone()),
        }
    }

    /// Sends a request that must succeed.
    pub async fn ok(&mut self, method: &str, params: Value) -> Value {
        match self.request(method, params).await {
            Ok(r) => r,
            Err(e) => panic!("{method} failed: {e}"),
        }
    }

    /// Sends a request that must fail; returns its error code.
    pub async fn fails(&mut self, method: &str, params: Value) -> String {
        match self.request(method, params).await {
            Ok(r) => panic!("{method} unexpectedly succeeded: {r}"),
            Err(e) => e["code"].as_str().unwrap().to_string(),
        }
    }

    /// The next event notification, from those already received or the next frames.
    pub async fn next_event(&mut self) -> Value {
        if !self.events.is_empty() {
            return self.events.remove(0);
        }
        loop {
            let v = self.recv().await.expect("connection open");
            if v["method"] == "pwc.event" {
                return v["params"].clone();
            }
        }
    }

    /// Fails if an event arrives within a short time.
    pub async fn no_event(&mut self) {
        assert!(self.events.is_empty(), "unexpected events {:?}", self.events);
        let mut line = Vec::new();
        let got = tokio::time::timeout(Duration::from_millis(200), self.reader.read_until(b'\n', &mut line)).await;
        assert!(got.is_err(), "unexpected frame {}", String::from_utf8_lossy(&line));
    }

    pub async fn hello(&mut self) -> Value {
        self.ok("local.hello", json!({"protocols": ["pwc-local/1"]})).await
    }

    pub async fn authenticate(&mut self, hello: &Value, client_id: &str, secret: &[u8; 32]) -> Value {
        let sn: [u8; 32] = bitcoin::hex::FromHex::from_hex(hello["server_nonce"].as_str().unwrap()).unwrap();
        let cn = [7u8; 32];
        let proof = local::client_proof(secret, &sn, &cn, client_id);
        let hex = |b: &[u8]| bitcoin::hex::DisplayHex::to_lower_hex_string(b);
        let v = self
            .call("local.authenticate", json!({"client_id": client_id, "client_nonce": hex(&cn), "client_proof": hex(&proof)}))
            .await;
        if let Some(r) = v.get("result") {
            let expected = local::server_proof(secret, &sn, &cn, client_id);
            assert_eq!(r["server_proof"], json!(hex(&expected)));
        }
        v
    }
}

/// The choice a User makes when approving exactly what was asked: requested network or regtest, and the
/// requested resources or every wallet of the network.
pub fn as_requested(backend: &MemoryBackend, r: &PermissionRequest) -> GrantChoice {
    use pwc_provider::backend::WalletBackend;
    let network = r.network.clone().unwrap_or_else(|| Network("regtest".into()));
    let (wallet_ids, account_ids) = if r.wallet_ids.is_empty() && r.account_ids.is_empty() {
        (backend.wallets().into_iter().filter(|w| w.network == network.as_str()).map(|w| w.wallet_id).collect(), vec![])
    } else {
        (r.wallet_ids.clone(), r.account_ids.clone())
    };
    GrantChoice { network, scopes: r.scopes.clone(), wallet_ids, account_ids, expires_at: r.expires_at, constraints: r.constraints.clone() }
}

impl Harness {
    /// Answers the next prompt, which must be a permission request, with `choose`.
    pub async fn answer_permissions(&mut self, choose: impl FnOnce(&PermissionRequest) -> Option<GrantChoice>) -> PermissionRequest {
        match self.next_prompt().await {
            Prompt::Permissions { request, reply } => {
                let _ = reply.send(choose(&request));
                request
            }
            _ => panic!("expected a permission prompt"),
        }
    }

    /// Requests `params` (scopes and so on) and approves them as requested; returns the completed Operation.
    pub async fn grant(&mut self, c: &mut Conn, mut params: Value) -> Value {
        if params.get("idempotency_key").is_none() {
            params["idempotency_key"] = json!(format!("k-{}", rand_suffix()));
        }
        let op = c.ok("session.requestPermissions", params).await;
        let backend = self.backend.clone();
        self.answer_permissions(|r| Some(as_requested(&backend, r))).await;
        let done = c.wait(op["operation_id"].as_str().unwrap()).await;
        assert_eq!(done["state"], "completed", "{done}");
        done
    }

    /// Pairs a Client and grants it `scopes` on every regtest wallet.
    pub async fn client(&mut self, scopes: &[&str]) -> (Conn, String, [u8; 32]) {
        let (mut c, id, secret) = self.pair().await;
        if !scopes.is_empty() {
            self.grant(&mut c, json!({"scopes": scopes})).await;
            if scopes.contains(&"events:read") {
                assert_eq!(c.next_event().await["event_type"], "operation.stateChanged");
            }
        }
        (c, id, secret)
    }
}

pub fn rand_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!("{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed))
}

impl Conn {
    /// Polls `operation.get` until the Operation is terminal.
    pub async fn wait(&mut self, op_id: &str) -> Value {
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let op = self.ok("operation.get", json!({"operation_id": op_id})).await;
            let s = op["state"].as_str().unwrap();
            if ["completed", "rejected", "expired", "canceled", "failed"].contains(&s) {
                return op;
            }
            assert!(tokio::time::Instant::now() < deadline, "Operation {op_id} still {s}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

pub fn code(v: &Value) -> &str {
    v["error"]["data"]["code"].as_str().unwrap_or("")
}
