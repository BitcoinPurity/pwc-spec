//! TASK-023: backend calls are made without the state lock and never run concurrently.

mod common;

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::Duration;

use common::{Conn, Harness, WAIT};
use pwc_provider::backend::*;
use pwc_provider::error::PwcError;
use pwc_provider::memory::MemoryBackend;
use pwc_provider::provider::{Config, Provider};
use pwc_provider::types::{Id, MessageFormat, Text};
use serde_json::{json, Value};

const ACCOUNT: &str = "a-regtest-wpkh";

/// The in-memory backend, with `balance` able to wait, call `notify` or panic, and a count of concurrent calls.
struct Probe {
    memory: Arc<MemoryBackend>,
    hold: Mutex<bool>,
    released: Condvar,
    entered: AtomicBool,
    notify: OnceLock<Weak<Provider>>,
    panic: AtomicBool,
    active: AtomicUsize,
    max: AtomicUsize,
}

impl Probe {
    fn call<T>(&self, f: impl FnOnce() -> T) -> T {
        let n = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(n, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(1));
        let v = f();
        self.active.fetch_sub(1, Ordering::SeqCst);
        v
    }

    fn set_hold(&self, hold: bool) {
        *self.hold.lock().unwrap() = hold;
        self.released.notify_all();
    }
}

impl WalletBackend for Probe {
    fn wallets(&self) -> Vec<WalletInfo> {
        self.call(|| self.memory.wallets())
    }
    fn account(&self, account_id: &str) -> Option<AccountInfo> {
        self.call(|| self.memory.account(account_id))
    }
    fn balance(&self, account_id: &str, reserved: &Reserved) -> Result<Balance, PwcError> {
        self.call(|| {
            assert!(!self.panic.load(Ordering::SeqCst), "backend panic");
            if let Some(p) = self.notify.get().and_then(Weak::upgrade) {
                p.notify(BackendEvent::BalanceChanged {
                    account_id: Id(account_id.into()),
                });
            }
            self.entered.store(true, Ordering::SeqCst);
            let mut hold = self.hold.lock().unwrap();
            while *hold {
                hold = self.released.wait(hold).unwrap();
            }
            drop(hold);
            self.memory.balance(account_id, reserved)
        })
    }
    fn receive_address(&self, account_id: &str) -> Result<AddressRecord, PwcError> {
        self.call(|| self.memory.receive_address(account_id))
    }
    fn create_address(&self, account_id: &str, label: Option<&Text>) -> Result<AddressRecord, PwcError> {
        self.call(|| self.memory.create_address(account_id, label))
    }
    fn addresses(&self, account_id: &str) -> Vec<AddressRecord> {
        self.call(|| self.memory.addresses(account_id))
    }
    fn utxos(&self, account_id: &str, reserved: &Reserved) -> Result<Vec<Utxo>, PwcError> {
        self.call(|| self.memory.utxos(account_id, reserved))
    }
    fn transactions(&self, account_id: &str) -> Result<Vec<TransactionRecord>, PwcError> {
        self.call(|| self.memory.transactions(account_id))
    }
    fn owns_script(&self, account_id: &str, script: &bitcoin::Script) -> bool {
        self.call(|| self.memory.owns_script(account_id, script))
    }
    fn is_unspent(&self, account_id: &str, outpoint: &bitcoin::OutPoint) -> bool {
        self.call(|| self.memory.is_unspent(account_id, outpoint))
    }
    fn fee_estimate(&self, network: &str, target_blocks: u64) -> Option<u64> {
        self.call(|| self.memory.fee_estimate(network, target_blocks))
    }
    fn create_psbt(
        &self,
        account_id: &str,
        request: &CreateRequest,
        excluded: &Reserved,
    ) -> Result<CreatedPsbt, PwcError> {
        self.call(|| self.memory.create_psbt(account_id, request, excluded))
    }
    fn owned_output(&self, account_id: &str, outpoint: &bitcoin::OutPoint) -> Option<bitcoin::TxOut> {
        self.call(|| self.memory.owned_output(account_id, outpoint))
    }
    fn sign_psbt(&self, account_id: &str, psbt: &mut bitcoin::Psbt, inputs: &[usize]) -> Result<Vec<u32>, PwcError> {
        self.call(|| self.memory.sign_psbt(account_id, psbt, inputs))
    }
    fn sign_message(
        &self,
        account_id: &str,
        script: &bitcoin::Script,
        message: &str,
        format: MessageFormat,
    ) -> Result<Vec<u8>, PwcError> {
        self.call(|| self.memory.sign_message(account_id, script, message, format))
    }
    fn broadcast(&self, network: &str, tx: &bitcoin::Transaction) -> Result<bitcoin::Txid, PwcError> {
        self.call(|| self.memory.broadcast(network, tx))
    }
}

/// Lets a held call go on even if the test fails, so that the runtime can shut down.
struct Release(Arc<Probe>);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.set_hold(false);
    }
}

async fn start() -> (Harness, Arc<Probe>) {
    let probe = Arc::new(Probe {
        memory: Arc::new(MemoryBackend::demo()),
        hold: Mutex::new(false),
        released: Condvar::new(),
        entered: AtomicBool::new(false),
        notify: OnceLock::new(),
        panic: AtomicBool::new(false),
        active: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
    });
    let (provider, prompts) = Provider::new(probe.clone(), Config::default());
    let h = Harness::restart(probe.memory.clone(), provider, prompts).await;
    (h, probe)
}

/// Runs a synchronous host call, failing the test instead of hanging if it blocks.
async fn returns<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::time::timeout(WAIT, tokio::task::spawn_blocking(f))
        .await
        .expect("the call was blocked")
        .unwrap()
}

async fn within<T>(what: &str, f: impl Future<Output = T>) -> T {
    tokio::time::timeout(WAIT, f)
        .await
        .unwrap_or_else(|_| panic!("{what} was blocked"))
}

async fn balance(c: &mut Conn) -> Result<Value, Value> {
    within(
        "account.getBalance",
        c.request("account.getBalance", json!({ "account_id": ACCOUNT })),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_waiting_backend_call_holds_up_only_backend_requests() {
    let (mut h, probe) = start().await;
    let (mut a, _, _) = h.client(&["balances:read"]).await;
    let (_, b_id, b_secret) = h.client(&["wallets:read"]).await;
    probe.set_hold(true);
    let _release = Release(probe.clone());
    let waiting = a
        .send_request("account.getBalance", json!({ "account_id": ACCOUNT }))
        .await;
    within("the backend call", async {
        while !probe.entered.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;

    within("local.authenticate", h.login(&b_id, &b_secret)).await;
    let p = h.provider.clone();
    assert_eq!(returns(move || p.paired_clients()).await.len(), 2);
    let p = h.provider.clone();
    let grants = returns(move || p.grants()).await;
    let p = h.provider.clone();
    assert!(!returns(move || p.save_state()).await.is_empty());
    let p = h.provider.clone();
    returns(move || {
        p.notify(BackendEvent::BalanceChanged {
            account_id: Id(ACCOUNT.into()),
        })
    })
    .await;
    let p = h.provider.clone();
    let b_grant = grants.iter().find(|g| g["client_id"] == b_id.as_str()).unwrap()["grant_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(returns(move || p.revoke_grant(&b_grant)).await);

    probe.set_hold(false);
    let r = within("the held request", a.response(waiting)).await;
    assert!(r.get("result").is_some(), "{r}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn notify_from_inside_a_backend_call() {
    let (mut h, probe) = start().await;
    let (mut c, _, _) = h.client(&["balances:read", "events:read"]).await;
    probe.notify.set(Arc::downgrade(&h.provider)).unwrap();
    balance(&mut c).await.expect("the balance");
    let e = within("the event", c.next_event()).await;
    assert_eq!(e["event_type"], "account.balanceChanged");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backend_calls_never_overlap() {
    let (mut h, probe) = start().await;
    let scopes = ["wallets:read", "balances:read", "utxos:read", "events:read"];
    let mut clients = Vec::new();
    for _ in 0..3 {
        clients.push(h.client(&scopes).await.0);
    }
    let mut tasks = Vec::new();
    for mut c in clients {
        tasks.push(tokio::spawn(async move {
            for _ in 0..10 {
                balance(&mut c).await.unwrap();
                within("utxo.list", c.ok("utxo.list", json!({ "account_id": ACCOUNT }))).await;
                within("wallet.list", c.ok("wallet.list", json!({}))).await;
            }
        }));
    }
    let p = h.provider.clone();
    let host = tokio::task::spawn_blocking(move || {
        for _ in 0..30 {
            p.notify(BackendEvent::BalanceChanged {
                account_id: Id(ACCOUNT.into()),
            });
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    for t in tasks {
        t.await.unwrap();
    }
    host.await.unwrap();
    assert_eq!(probe.max.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_backend_panic_is_answered() {
    let (mut h, probe) = start().await;
    let (mut c, _, _) = h.client(&["balances:read"]).await;
    probe.panic.store(true, Ordering::SeqCst);
    let e = balance(&mut c).await.expect_err("a panic");
    assert_eq!(e["code"], "INTERNAL_ERROR");
    probe.panic.store(false, Ordering::SeqCst);
    balance(&mut c).await.expect("served after the panic");
}
