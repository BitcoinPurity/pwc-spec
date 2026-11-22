# API contract: pwc-provider

This document states the interface of this implementation and the values it chooses where the specifications leave a choice. Field definitions, types and error-code meanings are those of `specs/PWC-Core-v0.2.1-EN.md`, `specs/PWC-Transport-Local-v0.1.0-EN.md` and `pwc-schema/`, and are not repeated here.

## 1. Transport

| Item | Value |
|---|---|
| Transport | LOCAL v0.1.0, protocol `pwc-local/1` |
| `provider_id` | `pwc-provider` |
| Endpoint | `<endpoint directory>/pwc-provider.sock`; directory 0700, socket 0600 |
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
