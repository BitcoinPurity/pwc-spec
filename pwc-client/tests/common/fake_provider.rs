//! A scripted LOCAL Provider for tests. It performs the real handshake, checks every frame it receives against
//! the schemas, and answers Core methods as the test's handler says.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pwc_client::local::{client_proof, server_proof};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{broadcast, mpsc};

use super::schema;

pub const PROVIDER_ID: &str = "fakewallet";
pub const CLIENT_ID: &str = "cl-test";
pub const SECRET: [u8; 32] = [7; 32];

pub enum Reply {
    Ok(Value),
    /// A Core error with this code.
    Err(&'static str),
    /// A JSON-RPC error without data, such as -32600.
    Rpc(i64),
    /// The result, sent after this many milliseconds.
    Later(u64, Value),
    /// No answer.
    Never,
    /// The Provider closes the connection without answering.
    Close,
}

pub type Handler = dyn Fn(&str, &Value) -> Reply + Send + Sync;

#[derive(Default)]
struct State {
    requests: Vec<(String, Value)>,
    problems: Vec<String>,
    closed: usize,
}

pub struct Fake {
    pub dir: tempfile::TempDir,
    pub path: PathBuf,
    state: Arc<Mutex<State>>,
    events: broadcast::Sender<Value>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.server.abort();
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    /// Answer `local.authenticate` with a wrong `server_proof`.
    pub wrong_server_proof: bool,
    pub max_frame_bytes: Option<u64>,
}

impl Fake {
    pub fn start(handler: impl Fn(&str, &Value) -> Reply + Send + Sync + 'static) -> Fake {
        Self::with_options(Options::default(), handler)
    }

    pub fn with_options(options: Options, handler: impl Fn(&str, &Value) -> Reply + Send + Sync + 'static) -> Fake {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join(format!("{PROVIDER_ID}.sock"));
        let listener = UnixListener::bind(&path).unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let (events, _) = broadcast::channel(1024);
        let handler: Arc<Handler> = Arc::new(handler);
        let server = tokio::spawn({
            let (state, events) = (state.clone(), events.clone());
            async move {
                loop {
                    let (stream, _) = listener.accept().await.unwrap();
                    tokio::spawn(serve(
                        stream,
                        options,
                        handler.clone(),
                        state.clone(),
                        events.subscribe(),
                    ));
                }
            }
        });
        Fake {
            dir,
            path,
            state,
            events,
            server,
        }
    }

    pub fn endpoint(&self) -> pwc_client::Endpoint {
        pwc_client::Endpoint {
            provider_id: PROVIDER_ID.into(),
            path: self.path.clone(),
        }
    }

    pub fn credential(&self) -> pwc_client::Credential {
        pwc_client::Credential {
            provider_id: PROVIDER_ID.into(),
            client_id: CLIENT_ID.into(),
            client_secret: hex(&SECRET),
        }
    }

    /// An authenticated Client.
    pub async fn client(&self) -> pwc_client::Client {
        let conn = pwc_client::Connection::open(&self.path).await.unwrap();
        conn.authenticate(&self.credential()).await.unwrap()
    }

    /// Sends a `pwc.event` notification on every authenticated connection.
    pub fn push_event(&self, event: Value) {
        schema::core("PwcEvent", &event).unwrap();
        let _ = self.events.send(event);
    }

    /// Every request received so far as (method, params). Fails if any frame broke a schema.
    pub fn requests(&self) -> Vec<(String, Value)> {
        let state = self.state.lock().unwrap();
        assert!(state.problems.is_empty(), "{:?}", state.problems);
        state.requests.clone()
    }

    /// The parameters of the only request for `method`.
    pub fn params(&self, method: &str) -> Value {
        let mut found: Vec<Value> = self
            .requests()
            .into_iter()
            .filter(|r| r.0 == method)
            .map(|r| r.1)
            .collect();
        assert_eq!(found.len(), 1, "requests for {method}");
        found.remove(0)
    }

    /// Waits until `n` connections have been closed by the Client.
    pub async fn closed(&self, n: usize) {
        for _ in 0..200 {
            if self.state.lock().unwrap().closed >= n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the Client did not close the connection");
    }
}

fn pwc_error(code: &str) -> Value {
    json!({"code": 1000, "message": format!("{code} from the fake"), "data": {"code": code, "message": format!("{code} from the fake")}})
}

async fn serve(
    stream: tokio::net::UnixStream,
    options: Options,
    handler: Arc<Handler>,
    state: Arc<Mutex<State>>,
    mut events: broadcast::Receiver<Value>,
) {
    let (read, mut write) = stream.into_split();
    let (out, mut outgoing) = mpsc::unbounded_channel::<Option<Value>>();
    let writer = tokio::spawn(async move {
        while let Some(Some(frame)) = outgoing.recv().await {
            schema::local("ProviderFrame", &frame).unwrap();
            let mut bytes = serde_json::to_vec(&frame).unwrap();
            bytes.push(b'\n');
            if write.write_all(&bytes).await.is_err() {
                break;
            }
        }
    });
    let server_nonce = [0x20u8; 32];
    let mut authenticated = false;
    let mut lines = BufReader::new(read).lines();
    loop {
        let line = tokio::select! {
            line = lines.next_line() => line,
            event = events.recv() => {
                if authenticated {
                    let _ = out.send(Some(json!({"jsonrpc": "2.0", "method": "pwc.event", "params": event.unwrap()})));
                }
                continue;
            }
        };
        let Ok(Some(line)) = line else { break };
        let frame: Value = serde_json::from_str(&line).unwrap();
        let method = frame["method"].as_str().unwrap_or("").to_string();
        let params = frame["params"].clone();
        {
            let mut state = state.lock().unwrap();
            if let Err(e) = schema::local("Request", &frame) {
                state.problems.push(e);
            }
            let def = format!("{method}.params");
            let checked = if method.starts_with("local.") {
                schema::local(&def, &params)
            } else if schema::core_defines(&def) {
                schema::core(&def, &params)
            } else {
                Ok(())
            };
            if let Err(e) = checked {
                state.problems.push(e);
            }
            state.requests.push((method.clone(), params.clone()));
        }
        let id = frame["id"].clone();
        let reply = match method.as_str() {
            "local.hello" => Reply::Ok(json!({
                "protocol": "pwc-local/1", "pwc_version": "0.2.1", "provider_id": PROVIDER_ID,
                "provider": {"name": "Fake Wallet", "version": "1.0.0"},
                "max_frame_bytes": options.max_frame_bytes.unwrap_or(1_048_576),
                "server_nonce": hex(&server_nonce), "delegation": false,
            })),
            "local.pair" => {
                authenticated = true;
                Reply::Ok(json!({"client_id": CLIENT_ID, "client_secret": hex(&SECRET), "session_id": "ss-1"}))
            }
            "local.authenticate" => {
                let cn = unhex(params["client_nonce"].as_str().unwrap());
                let expected = client_proof(&SECRET, &server_nonce, &cn, CLIENT_ID);
                if params["client_id"] != CLIENT_ID || params["client_proof"] != hex(&expected) {
                    Reply::Err("UNAUTHENTICATED")
                } else {
                    authenticated = true;
                    let mut proof = server_proof(&SECRET, &server_nonce, &cn, CLIENT_ID);
                    if options.wrong_server_proof {
                        proof[0] ^= 1;
                    }
                    Reply::Ok(json!({"session_id": "ss-2", "server_proof": hex(&proof)}))
                }
            }
            _ => handler(&method, &params),
        };
        let respond = |body: (&str, Value)| json!({"jsonrpc": "2.0", "id": id, body.0: body.1});
        match reply {
            Reply::Ok(result) => drop(out.send(Some(respond(("result", result))))),
            Reply::Err(code) => drop(out.send(Some(respond(("error", pwc_error(code)))))),
            Reply::Rpc(code) => drop(out.send(Some(respond((
                "error",
                json!({"code": code, "message": "Invalid Request"}),
            ))))),
            Reply::Later(ms, result) => {
                let (out, frame) = (out.clone(), respond(("result", result)));
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    let _ = out.send(Some(frame));
                });
            }
            Reply::Never => {}
            Reply::Close => {
                let _ = out.send(None);
                let _ = writer.await;
                return;
            }
        }
    }
    state.lock().unwrap().closed += 1;
}
