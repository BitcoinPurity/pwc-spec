# API contract: pwc-provider

This document states the interface of this implementation and the values it chooses where the specifications leave a choice. Field definitions, types and error-code meanings are those of `specs/PWC-Core-v0.2.1-EN.md`, `specs/PWC-Transport-Local-v0.1.0-EN.md` and `pwc-schema/`, and are not repeated here.

## 1. Transport

| Item | Value |
|---|---|
| Transport | LOCAL v0.1.0, protocol `pwc-local/1` |
| `provider_id` | The id passed to `local::bind` or `local::bind_pipe`, reported by `local.hello`; `PROVIDER_ID` (`pwc-provider`) for the demo binary |
| Endpoint | macOS and Linux: `<endpoint directory>/pwc-provider.sock`; directory 0700, socket 0600. Windows: `\\.\pipe\pwc-<sid>-pwc-provider`, first instance with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, byte mode, DACL `D:P(A;;GA;;;<sid>)` |
| `max_frame_bytes` | 1048576 |
| `delegation` | `false` |
| JSON-RPC errors | `-32700` for a parse error; `-32600` for an invalid request, a batch or a duplicate outstanding id; `1000` for everything else, with the complete `PwcError` as `data` and `message` equal to `data.message` |

### 1.1 `local.*` methods

| Method | Connection state | Notes |
|---|---|---|
| `local.hello` | New | Must be the first request. Without a common protocol: `UNSUPPORTED_OPERATION`, then close. Malformed parameters: `INVALID_ARGUMENT`, connection kept |
| `local.pair` | After hello | One pairing prompt per connection; a second request gets `RATE_LIMITED`. `delegator: true` gets `UNSUPPORTED_OPERATION` without a prompt |
| `local.authenticate` | After hello | One attempt per connection. Failure: `UNAUTHENTICATED` with fixed content, answered after 100 ms, then close. Malformed parameters: `INVALID_ARGUMENT`, then close |
| `local.unpair` | Authenticated | Answers `{"unpaired": true}`, then closes every connection of the Client |
| `local.openDelegate`, `local.delegated`, `local.closeDelegate` | Authenticated | `UNSUPPORTED_OPERATION` |

Other rules:

- First request on a new connection is not `local.hello`: `UNAUTHENTICATED`, then close.
- A Core method after hello but before authentication: `UNAUTHENTICATED`, connection kept.
- `local.hello` again, or `local.pair` or `local.authenticate` after authentication: `UNSUPPORTED_OPERATION`, connection kept.
- `local.authenticate` while pairing is in progress: `UNSUPPORTED_OPERATION`.
- An unknown `local.*` method after authentication: `UNSUPPORTED_METHOD`.
- A frame longer than `max_frame_bytes`: `PAYLOAD_TOO_LARGE` with `id` `null`, then close.
- Rate limits:
  - pairing prompts per peer process: at most 10 per 60 seconds;
  - failed authentications per peer process: at most 10 per 60 seconds;
  - over the limit: `RATE_LIMITED`; for authentication the connection is also closed.

## 2. Capabilities (`pwc.getCapabilities`)

| Field | Value |
|---|---|
| `pwc_version` | `0.2.1` |
| `provider` | `{"name":"PWC Provider","version":"<crate version>","platform":"desktop"}` |
| `profiles` | `pwc-core-read-v1`, `pwc-psbt-signer-v1` |
| `supported_networks` | `mainnet`, `signet`, `regtest` |
| `methods` | The 24 methods of section 3 |
| `features` | `coin_control`, `watch_only`, `event_push` |
| `scopes` | All 11 scopes of Core §6 |
| `script_types` | `p2wpkh`, `p2pkh` |
| `message_formats` | `bip322-simple`, `legacy` |
| `limits.max_page_size` | 100 |
| `limits.max_psbt_bytes` | 100000 |
| `limits.max_combine_psbts` | 5 |
| `limits.max_raw_transaction_bytes` | 400000 |
| `limits.max_message_bytes` | 65536 |
| `limits.max_concurrent_operations` | 8 |
| `limits.operation_retention_seconds` | 86400 |
| `limits.high_fee_rate` | `"500000"` (500 sat/vB) |

## 3. Methods

| Method | Scope | Returns | Notes |
|---|---|---|---|
| `pwc.getCapabilities` | — | Synchronous | |
| `session.getInfo` | — | Synchronous | |
| `session.requestPermissions` | — | Operation | Always prompts; no prompt-free shortcut. Unknown wallet or account ids are ignored silently |
| `session.getPermissions` | — | Synchronous | Every grant, including revoked and expired ones |
| `session.revokePermissions` | — | Synchronous, idempotent | |
| `wallet.list` | `wallets:read` | Synchronous | |
| `account.list` | `wallets:read` | Synchronous | |
| `account.getBalance` | `balances:read` | Synchronous | |
| `address.getReceiveAddress` | `addresses:read` | Synchronous | The first unused receive address |
| `address.create` | `addresses:create` | Synchronous, idempotent | |
| `address.list` | `addresses:read` | Synchronous, paginated | |
| `utxo.list` | `utxos:read` | Synchronous, paginated | |
| `transaction.list` | `transactions:read` | Synchronous, paginated | Newest first |
| `transaction.get` | `transactions:read` | Synchronous | |
| `psbt.create` | `psbt:create` | Operation, completed synchronously | Reserves the inputs for 10 minutes |
| `psbt.releaseReservation` | `psbt:create` | Synchronous, idempotent | |
| `psbt.sign` | `psbt:sign` | Operation | Approval of every transaction |
| `psbt.combine` | — | Synchronous | |
| `psbt.finalize` | — | Synchronous | |
| `transaction.broadcast` | `transactions:broadcast` | Operation, completed synchronously | |
| `message.sign` | `messages:sign` | Operation | Approval of every message |
| `message.verify` | — | Synchronous | |
| `operation.get` | — | Synchronous | |
| `operation.cancel` | — | Synchronous, idempotent | |

Methods in each account's `AccountSummary.methods`:

| Account | Unsupported methods |
|---|---|
| P2WPKH signing account | None |
| P2PKH signing account | `psbt.create`, `psbt.releaseReservation`, `psbt.sign` |
| P2WPKH watch-only account (`can_sign: false`) | `psbt.sign`, `message.sign` |

A method an account does not support fails with `UNSUPPORTED_OPERATION`.

## 4. Choices left open by the specifications

| Open point | Choice | Reference |
|---|---|---|
| Unknown parameter members | Rejected with `INVALID_ARGUMENT` | Core §3.1 |
| Operation-returning methods on a locked wallet | `WALLET_LOCKED` directly; `awaiting_unlock` is not used | Core §8 |
| Fee rate above `high_fee_rate`, undeterminable fee, nondefault sighash | `POLICY_VIOLATION` | Core §8.3 |
| Fee rate below the minimum of 1000 sat/kvB | `POLICY_VIOLATION` | Core §3.4 |
| `wallet_estimate` | The backend supplies a rate for `target_blocks`; `STATE_UNAVAILABLE` if it has no estimate | Core §8.3 |
| Method-specific check failures | Direct error, no Operation. Exceptions: a broadcast refused by the backend or an unreachable backend gives a `failed` Operation, and so does a failure after approval | Core §9.1 |
| Approval prompt rate | At most 10 per Client per 60 seconds, checked at check 9 | Core §12, item 13 |
| Approval time limit | 300 seconds | Core §9.1 |
| Pagination cursors | Stateless: an offset with an HMAC binding method, account and Client; misuse gives `INVALID_ARGUMENT` | Core §8 |
| Revoked and expired grants | Retained, so that `PERMISSION_REVOKED` can be told apart | Core §8, check 6 |
| Reservations | Implemented for 10 minutes; reserved UTXOs are reported as `locked` | Core §8.3 |
| A grant choice with no scope | Treated as a refusal; the Operation becomes `rejected` | — |
| A grant choice on another network than requested | Treated as a refusal | Core §8.1 |
| Operations awaiting approval when a snapshot is loaded | `failed`, `INTERNAL_ERROR`, `retryable: true` | — |
| Unfinished Operations when their grant is revoked or expires | `failed` with `PERMISSION_REVOKED` at once; the prompt is withdrawn | Core §7, step 6 |
| Unfinished Operations at `local.unpair` | The same, including `session.requestPermissions` | LOCAL §5.7 |

## 5. Signing and messages

### 5.1 Order of checks of `psbt.sign`

Before the order of Core §8.3, the account's support for `psbt.sign` is checked:

1. The account does not support `psbt.sign`: `UNSUPPORTED_OPERATION`
2. PSBT version other than 0: `UNSUPPORTED_PSBT_VERSION`; unparsable: `PSBT_VALIDATION_FAILED`
3. `input_indexes` duplicated or out of range: `INVALID_ARGUMENT`
4. A target input is not an output of the account on its network: `UNSUPPORTED_OPERATION`
5. Script other than P2WPKH, or a nonstandard sighash value: `UNSUPPORTED_SCRIPT`
6. Previous-output data in the PSBT differs from the wallet's, or outputs exceed inputs: `PSBT_VALIDATION_FAILED`
7. Net outflow above `max_amount_sats_per_request`: `GRANT_CONSTRAINT_VIOLATED`
8. Fee rate too high, undeterminable fee, or a sighash other than `SIGHASH_ALL`: `POLICY_VIOLATION`

Inputs that already carry the account's signature are not signed again. If only such inputs are targeted, the result is a success with an empty `signed_input_indexes`.

### 5.2 Message signatures

| Format | Address type | Signature encoding |
|---|---|---|
| `bip322-simple` | P2WPKH | BIP-322 simple: the consensus-encoded witness stack, in base64 |
| `legacy` | P2PKH | The 65-byte compact signature in base64, with the prefix `Bitcoin Signed Message:\n` |

Order of checks of `message.sign`: account support → address parses (else `INVALID_ARGUMENT`) → address network (else `NETWORK_MISMATCH`) → format supported and matching the address type (else `UNSUPPORTED_OPERATION`) → address owned by the account (else `INVALID_ARGUMENT`).

Order of checks of `message.verify`: network supported (else `UNSUPPORTED_OPERATION`) → address parses → address network → format and address type → signature decodes (else `INVALID_ARGUMENT`) → `valid`.

## 6. Demo data

`MemoryBackend::demo()` creates, for each network `<net>` (`mainnet`, `signet`, `regtest`):

| Wallet | Account | Script | Can sign | Initial confirmed funds |
|---|---|---|---|---|
| `w-<net>-main`, software | `a-<net>-wpkh` | P2WPKH | Yes | Two UTXOs of 1000000 and 500000 sat |
| | `a-<net>-pkh` | P2PKH | Yes | 200000 sat |
| `w-<net>-watch`, watch-only | `a-<net>-watch` | P2WPKH | No | 300000 sat |

Each chain is at height 1000, and the `wallet_estimate` fee rate is 2000 sat/kvB (changed with `MemoryBackend::set_fee_estimate`). Keys derive from fixed seeds and are for testing only.

## 7. Library interface

```rust
let (provider, prompts) = Provider::new(backend, Config::default());
let (provider, prompts) = Provider::with_state(backend, Config::default(), &snapshot)?;
let snapshot: Vec<u8> = provider.save_state();

let endpoint = local::bind(&dir, "pwc-provider")?;
local::serve(provider.clone(), endpoint).await;

provider.notify(BackendEvent::BalanceChanged { account_id });   // the host reports a wallet change
provider.revoke_grant(&grant_id);                                // the User revokes a grant in the wallet
provider.unpair_client(&client_id);                              // the User unpairs a Client in the wallet
provider.paired_clients();                                       // the list of paired Clients (LOCAL §5.7)
```

## 8. Java Provider SDK

Module and package `org.bitcoinpurity.pwc.provider`, version 0.2.1. Sections 1 to 7 apply unchanged. "Nullable" means the field is `null` when the Rust field is `None`.

### 8.1 Provider and server

| Member | Rust counterpart | Notes |
|---|---|---|
| `Provider.create(WalletBackend, Config, PromptHandler)` | `Provider::new` | |
| `Provider.withState(WalletBackend, Config, PromptHandler, byte[])` | `Provider::with_state` | Throws `StateException` with kind `CORRUPT` or `UNSUPPORTED_VERSION` |
| `byte[] saveState()` | `save_state` | |
| `Config config()` | `config` | |
| `List<PairedClient> pairedClients()` | `paired_clients` | |
| `List<Grant> grants()` | `grants` | |
| `boolean revokeGrant(String grantId)` | `revoke_grant` | |
| `void unpairClient(String clientId)` | `unpair_client` | |
| `void notify(BackendEvent)` | `notify` | |
| `void close()` | dropping the `Arc` | Releases this handle; servers and connections keep their own references |
| `LocalServer.PROVIDER_ID` | `PROVIDER_ID` | `"pwc-provider"` |
| `Optional<Path> LocalServer.defaultDirectory()` | `local::default_directory` | Empty on Windows |
| `LocalServer.start(Provider)`, `start(Provider, String providerId)` | `local::bind` in the default directory, or `local::bind_pipe` on Windows; then `local::serve` | `local.hello` reports `providerId`, or `PROVIDER_ID` without it |
| `LocalServer.start(Provider, Path dir)`, `start(Provider, Path dir, String providerId)` | `local::bind` and `local::serve` | macOS and Linux; `UnsupportedOperationException` on Windows |
| `EndpointException` | `EndpointError` | Kind `UNSAFE_DIRECTORY`, `PATH_TOO_LONG`, `ALREADY_RUNNING`, `NAME_TAKEN` (Windows) or `IO`, and the path or pipe name |
| `Path path()`, `void close()` | `Endpoint::path`, dropping `serve` | The socket path or pipe name; `close` returns after the socket file is removed |

`Config` (record, `Config.defaults()`, `toBuilder()`): `providerName`, `providerVersion`, `limits`, `helloTimeout`, `authTimeout`, `pairingTimeout`, `approvalTimeout`, `reservationTtl` (`Duration`), `pairingLimit`, `authFailureLimit` (`RateLimit(int max, Duration window)`), `authFailureDelay`, `promptLimit`, `eventBuffer` (`int`). `Limits` (record, `Limits.defaults()`, `toBuilder()`): `maxPageSize`, `maxPsbtBytes`, `maxCombinePsbts`, `maxRawTransactionBytes`, `maxMessageBytes`, `maxConcurrentOperations`, `operationRetentionSeconds`, `highFeeRate` (`long`, sat/kvB).

`PairedClient(clientId, claimedName, claimedDescription nullable, Instant pairedAt, Instant lastConnectedAt, PeerInfo peer)`. `Grant(grantId, clientId, network, List<String> scopes, walletIds, accountIds, Instant createdAt, Instant expiresAt nullable, GrantStatus status, GrantConstraints constraints nullable)`; `GrantStatus`: `ACTIVE`, `REVOKED`, `EXPIRED`.

### 8.2 Prompts

`PromptHandler.onPrompt(Prompt)` is called on the thread `pwc-prompts`, one prompt at a time.

| Class | `request()` | Answer |
|---|---|---|
| `PairingPrompt` | `PairingRequest(claimedName, claimedDescription nullable, pairingCode, PeerInfo peer)` | `approve()`, `reject()` |
| `PermissionPrompt` | `PermissionRequest(Requester requester, List<String> scopes, network nullable, walletIds, accountIds, Instant expiresAt nullable, GrantConstraints constraints nullable)` | `grant(GrantChoice)`, `reject()` |
| `PsbtSignPrompt` | `PsbtSignRequest(requester, accountId, network, List<Integer> inputsToSign, int inputCount, List<OutputSummary> outputs, long feeSats, long feeRateSatPerKvb, byte[] psbt)` | `approve()`, `reject()` |
| `MessageSignPrompt` | `MessageSignRequest(requester, accountId, network, address, message, MessageFormat format)` | `approve()`, `reject()` |

- Every prompt: `boolean isWithdrawn()`; `toString()` is the Rust `Display` text.
- A second answer throws `IllegalStateException`. An answer to a withdrawn prompt has no effect.
- `PeerInfo(Integer pid nullable, Path executable nullable)`, `Requester(clientId, claimedName, PeerInfo peer)`, `GrantConstraints(Long maxAmountSatsPerRequest nullable, Instant validUntil nullable)`, `GrantChoice(network, List<String> scopes, walletIds, accountIds, Instant expiresAt nullable, GrantConstraints constraints nullable)`, `OutputSummary(address nullable, String scriptPubKey hex, long amountSats, boolean toThisAccount)`.

### 8.3 Backend

`WalletBackend`, one method per method of the Rust trait:

| Java | Rust |
|---|---|
| `List<WalletInfo> wallets()` | `wallets` |
| `Optional<AccountInfo> account(String accountId)` | `account` |
| `Balance balance(String accountId, Set<OutPoint> reserved) throws PwcException` | `balance` |
| `AddressRecord receiveAddress(String accountId) throws PwcException` | `receive_address` |
| `AddressRecord createAddress(String accountId, String label) throws PwcException` | `create_address`; `label` nullable |
| `List<AddressRecord> addresses(String accountId)` | `addresses` |
| `List<Utxo> utxos(String accountId, Set<OutPoint> reserved) throws PwcException` | `utxos` |
| `List<TransactionRecord> transactions(String accountId) throws PwcException` | `transactions` |
| `boolean ownsScript(String accountId, byte[] scriptPubKey)` | `owns_script` |
| `boolean isUnspent(String accountId, OutPoint outpoint)` | `is_unspent` |
| `OptionalLong feeEstimate(String network, long targetBlocks)` | `fee_estimate` |
| `CreatedPsbt createPsbt(String accountId, CreateRequest request, Set<OutPoint> excluded) throws PwcException` | `create_psbt` |
| `Optional<TxOut> ownedOutput(String accountId, OutPoint outpoint)` | `owned_output` |
| `SignedPsbt signPsbt(String accountId, byte[] psbt, List<Integer> inputs) throws PwcException` | `sign_psbt` |
| `byte[] signMessage(String accountId, byte[] scriptPubKey, String message, MessageFormat format) throws PwcException` | `sign_message` |
| `String broadcast(String network, byte[] transaction) throws PwcException` | `broadcast`; returns the txid |
| `default Optional<WalletInfo> wallet(String walletId)` | `wallet` |

Value types: `OutPoint(String txid, int vout)` and `TxOut(long valueSats, byte[] scriptPubKey)`; transaction ids are lower-case hex strings in display order. Records holding `byte[]` compare it by reference.

Records: `WalletInfo(walletId, displayName, network, WalletKind kind, WalletState state, List<String> accountIds)`, `AccountInfo(accountId, walletId, displayName, network, boolean canSign, ScriptType scriptType, List<String> methods)`, `Balance(accountId, network, BalanceComponents components, Instant updatedAt, SyncStatus sync)`, `BalanceComponents(confirmedSats, pendingSelfSats, pendingExternalSats, pendingOutgoingSats, unconfirmedDeltaSats, spendableSats, lockedSats, immatureSats)` (all `long`), `SyncStatus(SyncState state, Integer tipHeight nullable)`, `AddressRecord(accountId, network, address, AddressRole role, AddressState state, label nullable)`, `Utxo(String txid, int vout, accountId, network, long valueSats, int confirmations, Integer blockHeight nullable, address, byte[] scriptPubKey, boolean spendable, List<UtxoRestriction> restrictions, boolean isChange)`, `TransactionRecord(accountId, network, String txid, TransactionStatus status, int confirmations, Integer blockHeight, Instant firstSeenAt, Instant confirmedAt, long netChangeSats, Long feeSats, String replacedByTxid)` (the last six nullable except `netChangeSats`), `CreateRequest(List<TxOut> outputs, long feeRate, List<OutPoint> selected nullable, boolean replaceable)`, `CreatedPsbt(byte[] psbt, long fee, long vsize, List<Integer> changeOutputIndexes, List<OutPoint> inputs)`, `SignedPsbt(byte[] psbt, List<Integer> signedInputs)`.

Enums: `WalletKind`, `WalletState`, `ScriptType`, `MessageFormat` (`BIP322_SIMPLE`, `BIP322_FULL`, `LEGACY`), `SyncState`, `AddressRole`, `AddressState`, `UtxoRestriction`, `TransactionStatus`, with the values of the strict schema; `ErrorCode` with the 36 codes of Core §10.

`BackendEvent` (sealed): `WalletLocked(walletId)`, `WalletUnlocked(walletId)`, `WalletSyncChanged(walletId)`, `BalanceChanged(accountId)`, `TransactionChanged(accountId, String txid, TransactionStatus status)`.

`PwcException(ErrorCode code, String message)` and `(code, message, String details nullable JSON text, Boolean retryable nullable)`.

`MemoryBackend` (`demo()`): `addWallet`, `addAccount`, `setWalletState`, `receive`, `receiveCoinbase`, `receiveConflicting`, `replace`, `dropPending`, `mine`, `reorg`, `setOffline`, `setFeeEstimate(String, OptionalLong)`, `spendByUser`, `tip`, plus `WalletBackend`. An unknown identifier throws `IllegalStateException` and leaves the in-memory backend unusable, as the Rust `expect` would.

### 8.4 Demo program

`java --enable-native-access=ALL-UNNAMED -cp <classpath> org.bitcoinpurity.pwc.provider.demo.Main [--dir DIR] [--approve ask|auto] [--approve-delay-ms N]`, with the behavior of section 2.4 of the product specification. `./gradlew run --args="--approve auto"` runs it; `./gradlew installDist` writes the start script `build/install/pwc-provider-java/bin/pwc-provider-java`, which uses `JAVA_HOME`.

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

# Python binding of the Client (pwc-client-ffi)

The Python module `pwc_client` has the interface of the Client section above, with these differences.

| Rust | Python |
|---|---|
| `endpoint_directory() -> PathBuf` | `endpoint_directory() -> str` |
| `discover(&Path) -> Vec<Endpoint>` | `discover(dir: str) -> list[Endpoint]`; `Endpoint.path` is a `str` |
| `Client::connect(&endpoint, store).await` | `await Client.connect(endpoint, store)` |
| `Client::pair(&endpoint, store, params).await` | `await Client.pair(endpoint, store, params)` |
| `client.request(method, serde_json::Value)` | `await client.request(method, dict)`, returning the result as Python objects |
| `Duration` | `datetime.timedelta` |
| `Option<T>` | `T` or `None` |
| `Result<T, Error>` | Returns `T` or raises a subclass of `Error` |
| `EventStream::next().await` | `await stream.next()` |
| `impl CredentialStore for T` | A subclass of `CredentialStore` with `load(provider_id)`, `save(credential)`, `delete(provider_id)` |
| `FileCredentialStore::new(dir)` | `file_credential_store(dir)` |
| `Connection`, `client_proof`, `server_proof` | Not available |

Variants of `Error` and their values:

| Variant | Values |
|---|---|
| `Error.Pwc` | `e[0]`: the `PwcError` |
| `Error.Rpc` | `e.code`, `e.message` |
| `Error.UnsafeEndpoint`, `Error.Protocol`, `Error.Io` | `e[0]`: the text |
| The others | None |
