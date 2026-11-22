# API contract

# PWC LOCAL Client (Rust)

This section states the public interface of `pwc-client` and the values it chooses where the specifications leave a choice. Field definitions, types and error-code meanings are those of `specs/PWC-Core-v0.2.1-EN.md`, `specs/PWC-Transport-Local-v0.1.0-EN.md` and `pwc-schema/`, and are not repeated here.

## 1. Discovery and handshake

| Item | Signature | Notes |
|---|---|---|
| Endpoint directory | `endpoint_directory() -> Result<PathBuf>` | LOCAL §3.2 |
| Discovery | `discover(dir) -> Result<Vec<Endpoint>>` | `Endpoint { provider_id, path }`, sorted by `provider_id`; a missing directory gives an empty list |
| Open | `Connection::open(path) -> Result<Connection>` | Directory check, connect, `local.hello` with `pwc-local/1` |
| Provider information | `Connection::hello() -> HelloInfo` | Unauthenticated until `authenticate` succeeds |
| Pairing code | `pairing_code() -> String` | 6 decimal digits |
| Pair | `Connection::pair(PairParams) -> Result<(Client, Credential)>` | Returns when the User decides |
| Authenticate | `Connection::authenticate(&Credential) -> Result<Client>` | One attempt per connection |

`Connection::open` fails with `Protocol` if the Provider chooses another protocol, sends a malformed `server_nonce`, or reports a `provider_id` different from the socket's file name.

## 2. Credential store

```rust
pub trait CredentialStore: Send + Sync {
    fn load(&self, provider_id: &str) -> Result<Option<Credential>, Error>;
    fn save(&self, credential: &Credential) -> Result<(), Error>;   // replaces
    fn delete(&self, provider_id: &str) -> Result<(), Error>;
}
```

| Item | Signature | Notes |
|---|---|---|
| Connect | `Client::connect(&Endpoint, Arc<dyn CredentialStore>) -> Result<Client>` | `NotPaired` without a stored credential, before anything is sent |
| Pair | `Client::pair(&Endpoint, Arc<dyn CredentialStore>, PairParams) -> Result<Client>` | Saves the credential |
| Unpair | `Client::unpair() -> Result<()>` | `local.unpair`, then deletes the stored credential |
| File store | `FileCredentialStore::new(dir)` | `<dir>/<provider_id>.json`, file 0600, directory 0700 |

`Credential { provider_id, client_id, client_secret }`; `client_secret` is 64 hexadecimal characters and is hidden in `Debug` output.

## 3. Core methods

Every function is `async`, takes `&self` and returns `Result<_, Error>`.

| Method | Function | Returns |
|---|---|---|
| `pwc.getCapabilities` | `get_capabilities()` | `Capabilities` |
| `session.getInfo` | `get_session_info()` | `SessionInfo` |
| `session.requestPermissions` | `request_permissions(RequestPermissionsParams)` | `Operation` |
| `session.getPermissions` | `get_permissions()` | `Vec<Grant>` |
| `session.revokePermissions` | `revoke_permissions(grant_id, idempotency_key)` | `bool` |
| `wallet.list` | `list_wallets()` | `Vec<WalletSummary>` |
| `account.list` | `list_accounts(wallet_id)` | `Vec<AccountSummary>` |
| `account.getBalance` | `get_balance(account_id)` | `Balance` |
| `address.getReceiveAddress` | `get_receive_address(account_id)` | `AddressRecord` |
| `address.create` | `create_address(account_id, label, idempotency_key)` | `AddressRecord` |
| `address.list` | `list_addresses(account_id, cursor, page_size)` | `AddressPage` |
| `utxo.list` | `list_utxos(account_id, cursor, page_size)` | `UtxoPage` |
| `transaction.list` | `list_transactions(account_id, cursor, page_size)` | `TransactionPage` |
| `transaction.get` | `get_transaction(account_id, txid)` | `TransactionRecord` |
| `psbt.create` | `create_psbt(CreatePsbtParams)` | `Operation` |
| `psbt.releaseReservation` | `release_reservation(account_id, reservation_id, idempotency_key)` | `bool` |
| `psbt.sign` | `sign_psbt(SignPsbtParams)` | `Operation` |
| `psbt.combine` | `combine_psbts(psbts)` | `PsbtObject` |
| `psbt.finalize` | `finalize_psbt(psbt)` | `FinalizePsbtResult` |
| `transaction.broadcast` | `broadcast_transaction(account_id, raw_transaction, idempotency_key)` | `Operation` |
| `message.sign` | `sign_message(SignMessageParams)` | `Operation` |
| `message.verify` | `verify_message(VerifyMessageParams)` | `bool` |
| `operation.get` | `get_operation(operation_id)` | `Operation` |
| `operation.cancel` | `cancel_operation(operation_id, idempotency_key)` | `Operation` |

`request(method, params) -> Result<Value>` sends any method, including `vendor.*` extensions and the delegation methods of LOCAL §5.8. `params` must be a JSON object.

Result of a terminal Operation:

| Method of the Operation | Accessor | Returns |
|---|---|---|
| `session.requestPermissions` | `permissions_result()` | `Vec<Grant>` |
| `psbt.create` | `create_psbt_result()` | `CreatePsbtResult` |
| `psbt.sign` | `sign_psbt_result()` | `SignPsbtResult` |
| `transaction.broadcast` | `broadcast_result()` | `String` (txid) |
| `message.sign` | `sign_message_result()` | `SignMessageResult` |

An accessor returns `Pwc(error)` for an Operation in state `rejected`, `expired`, `canceled` or `failed`, `OperationPending` for a nonterminal one, and `ResultUnavailable` for a completed one whose result the Provider withholds (Core §4.8).

## 4. Helpers

| Item | Signature | Notes |
|---|---|---|
| Idempotency key | `idempotency_key() -> String` | 32 hexadecimal characters from 16 random bytes |
| Wait | `wait_operation(operation_id, timeout) -> Result<Operation>` | `Timeout` at the limit; any error of `operation.get` is returned |
| Events | `subscribe_events() -> EventStream`; `EventStream::next() -> Result<PwcEvent>` | Events from the time of subscription |
| All pages | `list_all_addresses`, `list_all_utxos`, `list_all_transactions` `(account_id)` | Default page size; `CURSOR_EXPIRED` is returned to the caller |
| Close | `close()` | Later requests fail with `ConnectionClosed` |

## 5. Errors

| `Error` | Meaning |
|---|---|
| `Pwc(PwcError)` | The Provider's error (JSON-RPC code `1000`), or the error of a terminal Operation |
| `Rpc { code, message }` | `-32700` or `-32600` |
| `UnsafeEndpoint` | The endpoint directory failed the check of LOCAL §3.2, or `HOME` is not set |
| `NotPaired` | No stored credential |
| `ProviderAuthFailed` | Wrong `server_proof`; the connection was closed |
| `ConnectionClosed` | The connection ended; outstanding requests get this error |
| `FrameTooLarge` | The request exceeds `max_frame_bytes`; nothing was sent |
| `Protocol` | A response that does not have the specified shape; `params` that is not an object |
| `Io` | Socket or credential file error |
| `Timeout`, `OperationPending`, `ResultUnavailable`, `EventsMissed` | See sections 3 and 4 |

`ErrorCode` has the 36 codes of Core §10 and `Other(String)` for any other code.

## 6. Choices left open by the specifications

| Open point | Choice | Reference |
|---|---|---|
| JSON-RPC `id` | Integers from 1, counted per connection | LOCAL §4.2 |
| Largest frame accepted from the Provider | `max_frame_bytes` (1048576 before `local.hello` is answered); a longer frame closes the connection | LOCAL §4.1 |
| A frame that is not JSON | Closes the connection | LOCAL §4.1 |
| A response whose `id` matches no outstanding request, including `id` `null` | Ignored | LOCAL §4.3 |
| Notifications other than `pwc.event` | Ignored | LOCAL §5.8 |
| Event buffer | 256 events per stream; a slower reader gets `EventsMissed` and continues with the newest 256 | LOCAL §8 |
| Polling interval of `wait_operation` | 1 second, or at once on the Operation's `operation.stateChanged` event | LOCAL §8, Client Guide §3 |
| Request time limit | None; `local.pair` waits for the User | LOCAL §5.3 |
| Amounts in a result that are not canonical | The result is refused with `Protocol` | Core §3.2 |
| Timestamps | Kept as text; not parsed | Core §3.2 |
