//! The authenticated session: a raw `request`, one typed function per Core method, and helpers.

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::time::Instant;

use crate::credential::CredentialStore;
use crate::error::Error;
use crate::local::{hex, random, Connection, Endpoint, Link};
use crate::types::*;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// A new random `Id` for an `idempotency_key`. Make one for each intended action and keep it until the action
/// is finished: every retry of that action, also after reconnecting, must carry the same key (Core §9.3).
pub fn idempotency_key() -> String {
    hex(&random::<16>())
}

struct Inner {
    link: Link,
    hello: HelloInfo,
    client_id: String,
    session_id: String,
    store: Option<Arc<dyn CredentialStore>>,
}

/// An authenticated session with a Provider. Cloning gives another handle to the same connection; any number
/// of requests may be outstanding at once.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

/// Events pushed by the Provider, read by pulling. It delivers the events that arrive after it was created.
pub struct EventStream {
    events: tokio::sync::Mutex<broadcast::Receiver<PwcEvent>>,
}

impl EventStream {
    /// The next event. `EventsMissed` means this reader fell behind and events were discarded: query the
    /// authoritative state again, then keep reading. `ConnectionClosed` ends the stream.
    pub async fn next(&self) -> Result<PwcEvent, Error> {
        match self.events.lock().await.recv().await {
            Ok(event) => Ok(event),
            Err(broadcast::error::RecvError::Lagged(_)) => Err(Error::EventsMissed),
            Err(broadcast::error::RecvError::Closed) => Err(Error::ConnectionClosed),
        }
    }
}

#[derive(Deserialize)]
struct Grants {
    grants: Vec<Grant>,
}

#[derive(Deserialize)]
struct Revoked {
    revoked: bool,
}

#[derive(Deserialize)]
struct Wallets {
    wallets: Vec<WalletSummary>,
}

#[derive(Deserialize)]
struct Accounts {
    accounts: Vec<AccountSummary>,
}

#[derive(Deserialize)]
struct Transaction {
    transaction: TransactionRecord,
}

#[derive(Deserialize)]
struct Released {
    released: bool,
}

#[derive(Deserialize)]
struct Valid {
    valid: bool,
}

fn listing(account_id: &str, cursor: Option<String>, page_size: Option<u64>) -> Value {
    let mut params = json!({"account_id": account_id});
    if let Some(cursor) = cursor {
        params["cursor"] = json!(cursor);
    }
    if let Some(page_size) = page_size {
        params["page_size"] = json!(page_size);
    }
    params
}

impl Client {
    pub(crate) fn new(link: Link, hello: HelloInfo, client_id: String, session_id: String) -> Client {
        Client {
            inner: Arc::new(Inner {
                link,
                hello,
                client_id,
                session_id,
                store: None,
            }),
        }
    }

    fn with_store(self, store: Arc<dyn CredentialStore>) -> Client {
        let inner = Arc::into_inner(self.inner).expect("a new Client has one handle");
        Client {
            inner: Arc::new(Inner {
                store: Some(store),
                ..inner
            }),
        }
    }

    /// Connects and authenticates with the credential `store` holds for the endpoint's Provider. `NotPaired`
    /// if it holds none; `UNAUTHENTICATED` if the Provider no longer accepts it, in which case pair again.
    pub async fn connect(endpoint: &Endpoint, store: Arc<dyn CredentialStore>) -> Result<Client, Error> {
        let credential = store.load(&endpoint.provider_id)?.ok_or(Error::NotPaired)?;
        let client = Connection::open(&endpoint.path)
            .await?
            .authenticate(&credential)
            .await?;
        Ok(client.with_store(store))
    }

    /// Connects, pairs and saves the new credential in `store`. Show `params.pairing_code` to the User before
    /// calling: this returns only when the User has decided in the wallet.
    pub async fn pair(
        endpoint: &Endpoint,
        store: Arc<dyn CredentialStore>,
        params: PairParams,
    ) -> Result<Client, Error> {
        let (client, credential) = Connection::open(&endpoint.path).await?.pair(params).await?;
        store.save(&credential)?;
        Ok(client.with_store(store))
    }

    /// What the Provider said in `local.hello`.
    pub fn hello(&self) -> HelloInfo {
        self.inner.hello.clone()
    }

    pub fn client_id(&self) -> String {
        self.inner.client_id.clone()
    }

    pub fn session_id(&self) -> String {
        self.inner.session_id.clone()
    }

    /// Sends one request and returns its `result`. `params` must be a JSON object.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, Error> {
        self.inner.link.request(method, params).await
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, Error> {
        let result = self.request(method, params).await?;
        serde_json::from_value(result).map_err(|e| Error::Protocol(format!("result of {method}: {e}")))
    }

    /// Closes the connection. Grants and Operations belong to the `client_id` and survive (LOCAL §5.5).
    pub async fn close(&self) {
        self.inner.link.close().await;
    }

    /// `local.unpair`: the Provider deletes the credential, revokes every grant and closes every connection of
    /// this Client. The credential is also deleted from the store given to `connect` or `pair`.
    pub async fn unpair(&self) -> Result<(), Error> {
        self.request("local.unpair", json!({})).await?;
        if let Some(store) = &self.inner.store {
            store.delete(&self.inner.hello.provider_id)?;
        }
        Ok(())
    }

    /// `pwc.getCapabilities`. Use only the methods, features and limits listed here (Core §4.1).
    pub async fn get_capabilities(&self) -> Result<Capabilities, Error> {
        self.call("pwc.getCapabilities", json!({})).await
    }

    /// `session.getInfo`.
    pub async fn get_session_info(&self) -> Result<SessionInfo, Error> {
        self.call("session.getInfo", json!({})).await
    }

    /// `session.requestPermissions`. The Operation completes when the User decides; read the grants with
    /// `Operation::permissions_result`.
    pub async fn request_permissions(&self, params: RequestPermissionsParams) -> Result<Operation, Error> {
        self.call("session.requestPermissions", json!(params)).await
    }

    /// `session.getPermissions`: this Client's grants, which may include revoked and expired ones.
    pub async fn get_permissions(&self) -> Result<Vec<Grant>, Error> {
        self.call::<Grants>("session.getPermissions", json!({}))
            .await
            .map(|r| r.grants)
    }

    /// `session.revokePermissions`. False if the grant was already revoked or expired.
    pub async fn revoke_permissions(&self, grant_id: &str, idempotency_key: &str) -> Result<bool, Error> {
        let params = json!({"grant_id": grant_id, "idempotency_key": idempotency_key});
        self.call::<Revoked>("session.revokePermissions", params)
            .await
            .map(|r| r.revoked)
    }

    /// `wallet.list` (scope `wallets:read`).
    pub async fn list_wallets(&self) -> Result<Vec<WalletSummary>, Error> {
        self.call::<Wallets>("wallet.list", json!({})).await.map(|r| r.wallets)
    }

    /// `account.list` (scope `wallets:read`).
    pub async fn list_accounts(&self, wallet_id: &str) -> Result<Vec<AccountSummary>, Error> {
        self.call::<Accounts>("account.list", json!({"wallet_id": wallet_id}))
            .await
            .map(|r| r.accounts)
    }

    /// `account.getBalance` (scope `balances:read`).
    pub async fn get_balance(&self, account_id: &str) -> Result<Balance, Error> {
        self.call("account.getBalance", json!({"account_id": account_id})).await
    }

    /// `address.getReceiveAddress` (scope `addresses:read`). It may return the same address repeatedly; use
    /// `create_address` for a fresh one.
    pub async fn get_receive_address(&self, account_id: &str) -> Result<AddressRecord, Error> {
        self.call("address.getReceiveAddress", json!({"account_id": account_id}))
            .await
    }

    /// `address.create` (scope `addresses:create`).
    pub async fn create_address(
        &self,
        account_id: &str,
        label: Option<String>,
        idempotency_key: &str,
    ) -> Result<AddressRecord, Error> {
        let mut params = json!({"account_id": account_id, "idempotency_key": idempotency_key});
        if let Some(label) = label {
            params["label"] = json!(label);
        }
        self.call("address.create", params).await
    }

    /// `address.list` (scope `addresses:read`): one page. Pass the previous page's `next_cursor` to continue.
    pub async fn list_addresses(
        &self,
        account_id: &str,
        cursor: Option<String>,
        page_size: Option<u64>,
    ) -> Result<AddressPage, Error> {
        self.call("address.list", listing(account_id, cursor, page_size)).await
    }

    /// `utxo.list` (scope `utxos:read`): one page.
    pub async fn list_utxos(
        &self,
        account_id: &str,
        cursor: Option<String>,
        page_size: Option<u64>,
    ) -> Result<UtxoPage, Error> {
        self.call("utxo.list", listing(account_id, cursor, page_size)).await
    }

    /// `transaction.list` (scope `transactions:read`): one page, newest first.
    pub async fn list_transactions(
        &self,
        account_id: &str,
        cursor: Option<String>,
        page_size: Option<u64>,
    ) -> Result<TransactionPage, Error> {
        self.call("transaction.list", listing(account_id, cursor, page_size))
            .await
    }

    /// `transaction.get` (scope `transactions:read`).
    pub async fn get_transaction(&self, account_id: &str, txid: &str) -> Result<TransactionRecord, Error> {
        let params = json!({"account_id": account_id, "txid": txid});
        self.call::<Transaction>("transaction.get", params)
            .await
            .map(|r| r.transaction)
    }

    /// `psbt.create` (scope `psbt:create`). Read the result with `Operation::create_psbt_result`. If it has a
    /// `reservation_id` and the transaction is abandoned, call `release_reservation`.
    pub async fn create_psbt(&self, params: CreatePsbtParams) -> Result<Operation, Error> {
        self.call("psbt.create", json!(params)).await
    }

    /// `psbt.releaseReservation` (scope `psbt:create`). False if the reservation had already ended.
    pub async fn release_reservation(
        &self,
        account_id: &str,
        reservation_id: &str,
        idempotency_key: &str,
    ) -> Result<bool, Error> {
        let params =
            json!({"account_id": account_id, "reservation_id": reservation_id, "idempotency_key": idempotency_key});
        self.call::<Released>("psbt.releaseReservation", params)
            .await
            .map(|r| r.released)
    }

    /// `psbt.sign` (scope `psbt:sign`). The User approves in the wallet; read the result with
    /// `Operation::sign_psbt_result`. Signing never finalizes or broadcasts.
    pub async fn sign_psbt(&self, params: SignPsbtParams) -> Result<Operation, Error> {
        self.call("psbt.sign", json!(params)).await
    }

    /// `psbt.combine`: merges PSBTs (base64) of the same transaction.
    pub async fn combine_psbts(&self, psbts: Vec<String>) -> Result<PsbtObject, Error> {
        self.call("psbt.combine", json!({"psbts": psbts})).await
    }

    /// `psbt.finalize`. `raw_transaction` and `txid` are present when `complete` is true.
    pub async fn finalize_psbt(&self, psbt: &str) -> Result<FinalizePsbtResult, Error> {
        self.call("psbt.finalize", json!({"psbt": psbt})).await
    }

    /// `transaction.broadcast` (scope `transactions:broadcast`). Read the txid with
    /// `Operation::broadcast_result`.
    pub async fn broadcast_transaction(
        &self,
        account_id: &str,
        raw_transaction: &str,
        idempotency_key: &str,
    ) -> Result<Operation, Error> {
        let params =
            json!({"account_id": account_id, "raw_transaction": raw_transaction, "idempotency_key": idempotency_key});
        self.call("transaction.broadcast", params).await
    }

    /// `message.sign` (scope `messages:sign`). Read the result with `Operation::sign_message_result`.
    pub async fn sign_message(&self, params: SignMessageParams) -> Result<Operation, Error> {
        self.call("message.sign", json!(params)).await
    }

    /// `message.verify`. False for a well-formed signature that does not verify.
    pub async fn verify_message(&self, params: VerifyMessageParams) -> Result<bool, Error> {
        self.call::<Valid>("message.verify", json!(params))
            .await
            .map(|r| r.valid)
    }

    /// `operation.get`.
    pub async fn get_operation(&self, operation_id: &str) -> Result<Operation, Error> {
        self.call("operation.get", json!({"operation_id": operation_id})).await
    }

    /// `operation.cancel`. `OPERATION_NOT_CANCELABLE` once the Operation is past an irreversible step.
    pub async fn cancel_operation(&self, operation_id: &str, idempotency_key: &str) -> Result<Operation, Error> {
        let params = json!({"operation_id": operation_id, "idempotency_key": idempotency_key});
        self.call("operation.cancel", params).await
    }

    /// A stream of the events that arrive from now on. Events reach only a Client holding `events:read`.
    pub fn subscribe_events(&self) -> EventStream {
        EventStream {
            events: tokio::sync::Mutex::new(self.inner.link.subscribe()),
        }
    }

    /// Returns the Operation once it is terminal, or `Timeout` after `timeout`. It asks again when the
    /// Operation's `operation.stateChanged` event arrives, and otherwise once per second.
    pub async fn wait_operation(&self, operation_id: &str, timeout: Duration) -> Result<Operation, Error> {
        let deadline = Instant::now() + timeout;
        let events = self.subscribe_events();
        loop {
            let operation = self.get_operation(operation_id).await?;
            if operation.is_terminal() {
                return Ok(operation);
            }
            let changed = async {
                loop {
                    match events.next().await {
                        Ok(e) if e.operation_id.as_deref() != Some(operation_id) => {}
                        // Without events the timer below decides; a closed connection fails the next request.
                        Err(Error::ConnectionClosed) => std::future::pending().await,
                        _ => return,
                    }
                }
            };
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return Err(Error::Timeout),
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = changed => {}
            }
        }
    }

    /// Every address of the account, following `next_cursor` to the last page. `CURSOR_EXPIRED` means the
    /// listing must be started again.
    pub async fn list_all_addresses(&self, account_id: &str) -> Result<Vec<AddressRecord>, Error> {
        let mut all = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.list_addresses(account_id, cursor, None).await?;
            all.extend(page.addresses);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(all);
            }
        }
    }

    /// Every UTXO of the account, following `next_cursor` to the last page.
    pub async fn list_all_utxos(&self, account_id: &str) -> Result<Vec<Utxo>, Error> {
        let mut all = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.list_utxos(account_id, cursor, None).await?;
            all.extend(page.utxos);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(all);
            }
        }
    }

    /// Every transaction of the account, newest first, following `next_cursor` to the last page.
    pub async fn list_all_transactions(&self, account_id: &str) -> Result<Vec<TransactionRecord>, Error> {
        let mut all = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.list_transactions(account_id, cursor, None).await?;
            all.extend(page.transactions);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(all);
            }
        }
    }
}
