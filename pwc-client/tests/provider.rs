//! TASK-CLIENT-006: the library end to end against the Rust Provider of `pwc-provider/` and its in-memory wallet.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pwc_client::*;
use pwc_provider::backend::{BackendEvent, WalletBackend};
use pwc_provider::memory::MemoryBackend;
use pwc_provider::prompt::{GrantChoice, PermissionRequest, Prompt};
use pwc_provider::provider::{Config, Provider};
use pwc_provider::types::{Id, Network};

const WAIT: Duration = Duration::from_secs(10);
const ACCOUNT: &str = "a-regtest-wpkh";
const ALL_SCOPES: [&str; 11] = [
    "wallets:read",
    "balances:read",
    "addresses:read",
    "addresses:create",
    "utxos:read",
    "transactions:read",
    "psbt:create",
    "psbt:sign",
    "transactions:broadcast",
    "messages:sign",
    "events:read",
];

struct Wallet {
    dir: tempfile::TempDir,
    endpoint: Endpoint,
    provider: Arc<Provider>,
    backend: Arc<MemoryBackend>,
    /// What the User answers to signing prompts.
    approve_signing: Arc<AtomicBool>,
    /// While set, signing prompts are left unanswered.
    hold_signing: Arc<AtomicBool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.tasks.iter().for_each(|t| t.abort());
    }
}

/// The User granting exactly what was asked, on regtest and every wallet when the request names none.
fn as_requested(backend: &MemoryBackend, r: &PermissionRequest) -> GrantChoice {
    let network = r.network.clone().unwrap_or_else(|| Network("regtest".into()));
    let wallet_ids = backend
        .wallets()
        .into_iter()
        .filter(|w| w.network == network.as_str())
        .map(|w| w.wallet_id)
        .collect();
    GrantChoice {
        network,
        scopes: r.scopes.clone(),
        wallet_ids,
        account_ids: Vec::new(),
        expires_at: r.expires_at,
        constraints: r.constraints.clone(),
    }
}

async fn start() -> Wallet {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemoryBackend::demo());
    let (provider, mut prompts) = Provider::new(backend.clone(), Config::default());
    let bound = pwc_provider::local::bind(&dir.path().join("run"), pwc_provider::PROVIDER_ID).unwrap();
    let path = bound.path().to_path_buf();
    let approve_signing = Arc::new(AtomicBool::new(true));
    let hold_signing = Arc::new(AtomicBool::new(false));
    let server = tokio::spawn(pwc_provider::local::serve(provider.clone(), bound));
    let user = tokio::spawn({
        let (backend, approve, hold) = (backend.clone(), approve_signing.clone(), hold_signing.clone());
        async move {
            let mut held = Vec::new();
            while let Some(prompt) = prompts.recv().await {
                // The User takes a moment, so that an Operation is seen waiting before it ends.
                tokio::time::sleep(Duration::from_millis(200)).await;
                match prompt {
                    Prompt::Pairing { reply, .. } => {
                        let _ = reply.send(true);
                    }
                    Prompt::Permissions { request, reply } => {
                        let _ = reply.send(Some(as_requested(&backend, &request)));
                    }
                    Prompt::SignPsbt { reply, .. } | Prompt::SignMessage { reply, .. } => {
                        if hold.load(Ordering::SeqCst) {
                            held.push(reply);
                        } else {
                            let _ = reply.send(approve.load(Ordering::SeqCst));
                        }
                    }
                }
            }
        }
    });
    let endpoint = Endpoint {
        provider_id: pwc_provider::PROVIDER_ID.into(),
        path,
    };
    Wallet {
        dir,
        endpoint,
        provider,
        backend,
        approve_signing,
        hold_signing,
        tasks: vec![server, user],
    }
}

fn code<T: std::fmt::Debug>(result: Result<T, Error>) -> ErrorCode {
    match result {
        Err(Error::Pwc(e)) => e.code,
        other => panic!("expected a PwcError, got {other:?}"),
    }
}

fn sign_params(psbt: &str, key: &str) -> SignPsbtParams {
    SignPsbtParams {
        account_id: ACCOUNT.into(),
        psbt: psbt.into(),
        input_indexes: None,
        idempotency_key: key.into(),
    }
}

fn payment(address: &str, amount_sats: u64, key: &str) -> CreatePsbtParams {
    CreatePsbtParams {
        account_id: ACCOUNT.into(),
        outputs: vec![Payment {
            address: address.into(),
            amount_sats,
        }],
        fee_policy: FeePolicy::Rate { sat_per_kvb: 2000 },
        selected_utxos: None,
        replaceable: None,
        idempotency_key: key.into(),
    }
}

/// Pairs, stores the credential, connects again with it and obtains a grant for every scope.
async fn connect(wallet: &Wallet) -> (Client, Arc<dyn CredentialStore>) {
    let found = discover(wallet.endpoint.path.parent().unwrap()).unwrap();
    assert_eq!(found, vec![wallet.endpoint.clone()]);
    let store: Arc<dyn CredentialStore> = Arc::new(FileCredentialStore::new(wallet.dir.path().join("creds")));
    assert!(matches!(
        Client::connect(&found[0], store.clone()).await,
        Err(Error::NotPaired)
    ));
    let params = PairParams {
        client_name: "Test App".into(),
        client_description: None,
        pairing_code: pairing_code(),
    };
    let paired = Client::pair(&found[0], store.clone(), params).await.unwrap();
    paired.close().await;

    let client = Client::connect(&found[0], store.clone()).await.unwrap();
    assert_eq!(client.client_id(), paired.client_id());
    assert_eq!(client.get_session_info().await.unwrap().session_id, client.session_id());

    let op = client
        .request_permissions(RequestPermissionsParams {
            scopes: ALL_SCOPES.iter().map(|s| s.to_string()).collect(),
            network: None,
            wallet_ids: None,
            account_ids: None,
            expires_at: None,
            constraints: None,
            idempotency_key: idempotency_key(),
        })
        .await
        .unwrap();
    assert_eq!(op.state, OperationState::AwaitingApproval);
    assert_eq!(op.permissions_result(), Err(Error::OperationPending));
    let grants = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .permissions_result()
        .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(
        (grants[0].network.as_str(), grants[0].status),
        ("regtest", GrantStatus::Active)
    );
    assert_eq!(grants[0].scopes.len(), 11);
    assert_eq!(client.get_permissions().await.unwrap(), grants);
    (client, store)
}

#[tokio::test]
async fn capabilities_and_account_data() {
    let wallet = start().await;
    let (client, _) = connect(&wallet).await;

    let caps = client.get_capabilities().await.unwrap();
    assert_eq!(caps.pwc_version, "0.2.1");
    assert_eq!(caps.methods.len(), 24);
    assert!(caps.features.contains(&"event_push".to_string()));
    assert_eq!(client.hello().provider_id, "pwc-provider");

    let wallets = client.list_wallets().await.unwrap();
    let main = wallets.iter().find(|w| w.wallet_id == "w-regtest-main").unwrap();
    assert_eq!(
        (main.state, main.wallet_kind),
        (WalletState::Ready, WalletKind::Software)
    );
    let accounts = client.list_accounts("w-regtest-main").await.unwrap();
    let account = accounts.iter().find(|a| a.account_id == ACCOUNT).unwrap();
    assert!(account.can_sign && account.methods.contains(&"psbt.sign".to_string()));
    assert_eq!(
        code(client.list_accounts("w-mainnet-main").await),
        ErrorCode::WalletNotFound
    );

    let balance = client.get_balance(ACCOUNT).await.unwrap();
    assert_eq!((balance.confirmed_sats, balance.spendable_sats), (1_500_000, 1_500_000));
    assert_eq!(balance.sync.state, SyncState::Synced);

    let utxos = client.list_all_utxos(ACCOUNT).await.unwrap();
    let mut values: Vec<u64> = utxos.iter().map(|u| u.value_sats).collect();
    values.sort();
    assert_eq!(values, [500_000, 1_000_000]);
    assert!(utxos.iter().all(|u| u.spendable && u.restrictions.is_empty()));
    assert_eq!(client.list_utxos(ACCOUNT, None, Some(1)).await.unwrap().utxos.len(), 1);

    let receive = client.get_receive_address(ACCOUNT).await.unwrap();
    assert_eq!(
        (receive.role, receive.state),
        (AddressRole::Receive, AddressState::Unused)
    );
    let key = idempotency_key();
    let created = client
        .create_address(ACCOUNT, Some("Invoice 7".into()), &key)
        .await
        .unwrap();
    assert_eq!(created.label.as_deref(), Some("Invoice 7"));
    assert_eq!(
        client
            .create_address(ACCOUNT, Some("Invoice 7".into()), &key)
            .await
            .unwrap(),
        created
    );
    let addresses = client.list_all_addresses(ACCOUNT).await.unwrap();
    assert!(addresses.contains(&created));

    let history = client.list_all_transactions(ACCOUNT).await.unwrap();
    assert_eq!(history.len(), 2);
    let tx = client.get_transaction(ACCOUNT, &history[0].txid).await.unwrap();
    assert_eq!(tx, history[0]);
    assert_eq!(tx.status, TransactionStatus::Confirmed);
    assert_eq!(
        code(client.get_transaction(ACCOUNT, &"ab".repeat(32)).await),
        ErrorCode::TransactionNotFound
    );
}

#[tokio::test]
async fn pay_from_creation_to_broadcast() {
    let wallet = start().await;
    let (client, _) = connect(&wallet).await;
    let to = client.get_receive_address("a-regtest-pkh").await.unwrap().address;

    let created = client
        .create_psbt(payment(&to, 300_000, &idempotency_key()))
        .await
        .unwrap();
    let created = client
        .wait_operation(&created.operation_id, WAIT)
        .await
        .unwrap()
        .create_psbt_result()
        .unwrap();
    assert_eq!(created.fee_sats, (2000 * created.vsize).div_ceil(1000));
    assert!(created.reservation_id.is_some());

    let op = client
        .sign_psbt(sign_params(&created.psbt, &idempotency_key()))
        .await
        .unwrap();
    assert_eq!(op.state, OperationState::AwaitingApproval);
    let start = Instant::now();
    let signed = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .sign_psbt_result()
        .unwrap();
    // Approval takes 200 ms; ending well inside the one-second polling interval shows the event woke the wait.
    assert!(start.elapsed() < Duration::from_millis(800), "{:?}", start.elapsed());
    assert_eq!(signed.signing_status, SigningStatus::Complete);

    let done = client.finalize_psbt(&signed.psbt).await.unwrap();
    assert!(done.complete);
    let key = idempotency_key();
    let raw = done.raw_transaction.unwrap();
    let op = client.broadcast_transaction(ACCOUNT, &raw, &key).await.unwrap();
    let txid = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .broadcast_result()
        .unwrap();
    assert_eq!(Some(&txid), done.txid.as_ref());
    // A retry of the same action does not broadcast again: it returns the same Operation (Core §9.3).
    assert_eq!(
        client
            .broadcast_transaction(ACCOUNT, &raw, &key)
            .await
            .unwrap()
            .operation_id,
        op.operation_id
    );

    let tx = client.get_transaction(ACCOUNT, &txid).await.unwrap();
    assert_eq!(tx.status, TransactionStatus::Unconfirmed);
    assert_eq!(tx.fee_sats, Some(created.fee_sats));
    assert_eq!(tx.net_change_sats, -(300_000 + created.fee_sats as i64));
    let balance = client.get_balance(ACCOUNT).await.unwrap();
    assert_eq!(
        balance.confirmed_sats as i64 + balance.unconfirmed_delta_sats,
        1_500_000 + tx.net_change_sats
    );

    wallet.backend.mine("regtest", 1);
    assert_eq!(
        client.get_transaction(ACCOUNT, &txid).await.unwrap().status,
        TransactionStatus::Confirmed
    );
}

#[tokio::test]
async fn reservation_is_released() {
    let wallet = start().await;
    let (client, _) = connect(&wallet).await;
    let to = client.get_receive_address("a-regtest-pkh").await.unwrap().address;
    let op = client
        .create_psbt(payment(&to, 300_000, &idempotency_key()))
        .await
        .unwrap();
    let created = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .create_psbt_result()
        .unwrap();
    let reservation = created.reservation_id.unwrap();
    assert!(client.get_balance(ACCOUNT).await.unwrap().locked_sats > 0);

    assert!(client
        .release_reservation(ACCOUNT, &reservation, &idempotency_key())
        .await
        .unwrap());
    assert!(!client
        .release_reservation(ACCOUNT, &reservation, &idempotency_key())
        .await
        .unwrap());
    assert_eq!(client.get_balance(ACCOUNT).await.unwrap().locked_sats, 0);
    assert_eq!(
        code(
            client
                .release_reservation(ACCOUNT, "r-unknown", &idempotency_key())
                .await
        ),
        ErrorCode::ReservationNotFound
    );
}

#[tokio::test]
async fn sign_and_verify_a_message() {
    let wallet = start().await;
    let (client, _) = connect(&wallet).await;
    let address = client.get_receive_address(ACCOUNT).await.unwrap().address;
    let op = client
        .sign_message(SignMessageParams {
            account_id: ACCOUNT.into(),
            address: address.clone(),
            message: "Hello, wallet".into(),
            format: MessageFormat::Bip322Simple,
            idempotency_key: idempotency_key(),
        })
        .await
        .unwrap();
    let signed = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .sign_message_result()
        .unwrap();
    assert_eq!(
        (signed.address.as_str(), signed.format),
        (address.as_str(), MessageFormat::Bip322Simple)
    );

    let verify = |message: &str| VerifyMessageParams {
        network: "regtest".into(),
        address: address.clone(),
        message: message.into(),
        signature: signed.signature.clone(),
        format: MessageFormat::Bip322Simple,
    };
    assert!(client.verify_message(verify("Hello, wallet")).await.unwrap());
    assert!(!client.verify_message(verify("Hello, wallet!")).await.unwrap());
}

#[tokio::test]
async fn refusal_cancellation_and_idempotency() {
    let wallet = start().await;
    let (client, _) = connect(&wallet).await;
    let to = client.get_receive_address("a-regtest-pkh").await.unwrap().address;
    let op = client
        .create_psbt(payment(&to, 300_000, &idempotency_key()))
        .await
        .unwrap();
    let psbt = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .create_psbt_result()
        .unwrap()
        .psbt;

    wallet.approve_signing.store(false, Ordering::SeqCst);
    let op = client.sign_psbt(sign_params(&psbt, &idempotency_key())).await.unwrap();
    let refused = client.wait_operation(&op.operation_id, WAIT).await.unwrap();
    assert_eq!(refused.state, OperationState::Rejected);
    assert_eq!(code(refused.sign_psbt_result()), ErrorCode::UserRejected);

    wallet.hold_signing.store(true, Ordering::SeqCst);
    let key = idempotency_key();
    let op = client.sign_psbt(sign_params(&psbt, &key)).await.unwrap();
    assert_eq!(
        client
            .wait_operation(&op.operation_id, Duration::from_millis(300))
            .await,
        Err(Error::Timeout)
    );
    let mut other = sign_params(&psbt, &key);
    other.input_indexes = Some(vec![0]);
    assert_eq!(code(client.sign_psbt(other).await), ErrorCode::IdempotencyConflict);
    let canceled = client
        .cancel_operation(&op.operation_id, &idempotency_key())
        .await
        .unwrap();
    assert_eq!(canceled.state, OperationState::Canceled);
    assert_eq!(code(canceled.sign_psbt_result()), ErrorCode::OperationCanceled);
    assert_eq!(
        code(client.get_operation("op-unknown").await),
        ErrorCode::OperationNotFound
    );
}

/// L-20.
#[tokio::test]
async fn retry_after_reconnecting_returns_the_same_operation() {
    let wallet = start().await;
    let (client, store) = connect(&wallet).await;
    let to = client.get_receive_address("a-regtest-pkh").await.unwrap().address;
    let op = client
        .create_psbt(payment(&to, 300_000, &idempotency_key()))
        .await
        .unwrap();
    let psbt = client
        .wait_operation(&op.operation_id, WAIT)
        .await
        .unwrap()
        .create_psbt_result()
        .unwrap()
        .psbt;

    wallet.hold_signing.store(true, Ordering::SeqCst);
    let key = idempotency_key();
    let pending = client.sign_psbt(sign_params(&psbt, &key)).await.unwrap();
    client.close().await;
    assert_eq!(
        client.get_operation(&pending.operation_id).await,
        Err(Error::ConnectionClosed)
    );

    let again = Client::connect(&wallet.endpoint, store).await.unwrap();
    let retried = again.sign_psbt(sign_params(&psbt, &key)).await.unwrap();
    assert_eq!(retried.operation_id, pending.operation_id);
    assert_eq!(
        again.get_operation(&pending.operation_id).await.unwrap().state,
        OperationState::AwaitingApproval
    );
}

#[tokio::test]
async fn events_revocation_and_unpairing() {
    let wallet = start().await;
    let (client, store) = connect(&wallet).await;

    let events = client.subscribe_events();
    let account = Id(ACCOUNT.into());
    wallet.backend.receive(&account, 50_000, 0);
    wallet
        .provider
        .notify(BackendEvent::BalanceChanged { account_id: account });
    let event = tokio::time::timeout(WAIT, events.next()).await.unwrap().unwrap();
    assert_eq!(event.event_type, "account.balanceChanged");
    assert_eq!(event.account_id.as_deref(), Some(ACCOUNT));
    assert_eq!(client.get_balance(ACCOUNT).await.unwrap().pending_external_sats, 50_000);

    let grant = client.get_permissions().await.unwrap().remove(0);
    assert!(client
        .revoke_permissions(&grant.grant_id, &idempotency_key())
        .await
        .unwrap());
    assert!(!client
        .revoke_permissions(&grant.grant_id, &idempotency_key())
        .await
        .unwrap());
    assert_eq!(code(client.list_wallets().await), ErrorCode::PermissionRevoked);
    assert_eq!(
        code(client.revoke_permissions("g-unknown", &idempotency_key()).await),
        ErrorCode::GrantNotFound
    );
    assert_eq!(client.get_permissions().await.unwrap()[0].status, GrantStatus::Revoked);

    let credential = store.load("pwc-provider").unwrap().unwrap();
    client.unpair().await.unwrap();
    assert_eq!(store.load("pwc-provider").unwrap(), None);
    let conn = Connection::open(&wallet.endpoint.path).await.unwrap();
    assert_eq!(
        code(conn.authenticate(&credential).await.map(|_| ())),
        ErrorCode::Unauthenticated
    );
    assert!(wallet.provider.paired_clients().is_empty());
}
