# Purity Wallet Connect (PWC)
## Registry of Names and Identifiers

**Specification:** PWC-REGISTRY-001  
**Version:** 0.1.0 (review draft)  
**Date:** 2026-11-21  
**Language:** English  
**License:** CC BY 4.0  
**Covers:** PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001 v0.1.0; PWC-TRANSPORT-BROWSER-001 v0.1.0; PWC-TRANSPORT-REMOTE-001 v0.1.0; PWC-TRANSPORT-MOBILE-001 v0.1.0  
**Status:** Proposed; maintained under PWC-GOVERNANCE-001

> **Scope rule:** This registry lists every name and identifier that the PWC specifications define, so that implementations and extensions do not collide. It defines no behavior: each entry points to the section that does. A machine-readable copy is published alongside as `pwc-registry-0.1.0.json`; the two are generated from the same source.

## 1. How names are allocated

| Namespace | Who allocates | Procedure |
|---|---|---|
| Core and transport-profile names (`category.action` methods, scopes, features, error codes, event types, protocols, identifiers) | The specification maintainers | Only through a revision of the defining specification (PWC-GOVERNANCE-001 §4) |
| Extension names `vendor.<reverse-domain>.<name>` | Whoever controls the domain | No registration needed. The domain owner guarantees uniqueness (Core §11) |
| `provider_id` values (LOCAL §3.1, BROWSER §4.1, REMOTE §4.3) | The specification maintainers | First come, first served on request (PWC-GOVERNANCE-001 §5) |
| Network names | Fixed by the existing Purity networks | Only through a Core revision (Core §3.2) |

## 2. Methods

"Op" marks methods that return an Operation; "Key" marks methods that require an `idempotency_key` (Core §9.3).

| Method | Scope | Op | Key | Profile | Defined in |
|---|---|---|---|---|---|
| `pwc.getCapabilities` | none | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `session.getInfo` | none | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `session.requestPermissions` | none | Yes | Yes | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `session.getPermissions` | none | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `session.revokePermissions` | none | — | Yes | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `wallet.list` | `wallets:read` | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `account.list` | `wallets:read` | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.1 |
| `account.getBalance` | `balances:read` | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.2 |
| `address.getReceiveAddress` | `addresses:read` | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.2 |
| `address.create` | `addresses:create` | — | Yes | — | PWC-CORE-001 v0.2.1 §8.2 |
| `address.list` | `addresses:read` | — | — | — | PWC-CORE-001 v0.2.1 §8.2 |
| `utxo.list` | `utxos:read` | — | — | — | PWC-CORE-001 v0.2.1 §8.2 |
| `transaction.list` | `transactions:read` | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.2 |
| `transaction.get` | `transactions:read` | — | — | — | PWC-CORE-001 v0.2.1 §8.2 |
| `psbt.create` | `psbt:create` | Yes | Yes | — | PWC-CORE-001 v0.2.1 §8.3 |
| `psbt.releaseReservation` | `psbt:create` | — | Yes | — | PWC-CORE-001 v0.2.1 §8.3 |
| `psbt.sign` | `psbt:sign` | Yes | Yes | `pwc-psbt-signer-v1` | PWC-CORE-001 v0.2.1 §8.3 |
| `psbt.combine` | none | — | — | — | PWC-CORE-001 v0.2.1 §8.3 |
| `psbt.finalize` | none | — | — | — | PWC-CORE-001 v0.2.1 §8.3 |
| `transaction.broadcast` | `transactions:broadcast` | Yes | Yes | — | PWC-CORE-001 v0.2.1 §8.3 |
| `message.sign` | `messages:sign` | Yes | Yes | — | PWC-CORE-001 v0.2.1 §8.4 |
| `message.verify` | none | — | — | — | PWC-CORE-001 v0.2.1 §8.4 |
| `operation.get` | none | — | — | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.5 |
| `operation.cancel` | none | — | Yes | `pwc-core-read-v1` | PWC-CORE-001 v0.2.1 §8.5 |
| `local.hello` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.2 |
| `local.pair` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.3 |
| `local.authenticate` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.4 |
| `local.unpair` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.7 |
| `local.openDelegate` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.8 |
| `local.delegated` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.8 |
| `local.closeDelegate` | none | — | — | — | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.8 |
| `remote.ping` | none | — | — | — | PWC-TRANSPORT-REMOTE-001 v0.1.0 §6 |
| `remote.updateRelays` | none | — | — | — | PWC-TRANSPORT-REMOTE-001 v0.1.0 §6 |
| `remote.unpair` | none | — | — | — | PWC-TRANSPORT-REMOTE-001 v0.1.0 §6 |

The `local.*` methods exist only on LOCAL connections and the `remote.*` methods only on REMOTE pairings; they are never listed in `Capabilities.methods`.

## 3. Notifications

| Notification | Direction | Defined in |
|---|---|---|
| `pwc.event` | Provider to Client | PWC-TRANSPORT-LOCAL-001 v0.1.0 §8; PWC-TRANSPORT-REMOTE-001 v0.1.0 §5.3 |
| `local.delegatedEvent` | Provider to delegator | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.8 |
| `remote.paired` | Provider to Client | PWC-TRANSPORT-REMOTE-001 v0.1.0 §4.3 |
| `remote.pairingRejected` | Provider to Client | PWC-TRANSPORT-REMOTE-001 v0.1.0 §4.2 |
| `remote.relaysChanged` | Provider to Client | PWC-TRANSPORT-REMOTE-001 v0.1.0 §5.2 |

## 4. Scopes

| Scope | Grants access to | Approval behavior |
|---|---|---|
| `wallets:read` | Wallet/account inventory | Explicit initial disclosure consent |
| `balances:read` | `account.getBalance` | Grant required |
| `addresses:read` | Existing/current addresses | Grant required |
| `addresses:create` | New receive address | Explicit grant; no payment implied |
| `transactions:read` | Account transaction history | Grant required |
| `utxos:read` | Outpoints and coin-selection metadata | Privacy-sensitive grant |
| `psbt:create` | Construct PSBT using an account's resources; release its reservations | Grant plus local wallet policy; discloses the selected coins (Core §4.7) |
| `psbt:sign` | Request signatures for selected inputs | **Individual approval of every transaction** |
| `transactions:broadcast` | Submit raw transaction to wallet-selected backend | Grant plus applicable local policy |
| `messages:sign` | Request supported message signature | **Individual approval of every message** |
| `events:read` | Deliver authorized state-change events | Underlying read scopes still enforced |

## 5. Features

| Feature | Meaning |
|---|---|
| `coin_control` | `psbt.create` honors `selected_utxos` |
| `multisig` | The Provider handles multisignature accounts |
| `hardware_signing` | The Provider can sign through hardware devices |
| `watch_only` | The Provider handles watch-only accounts |
| `event_push` | The Provider delivers the events of Core §8.6 through a transport profile |

## 6. Error codes

Every code is a `PwcError.code` (Core §10). `PROVIDER_UNAVAILABLE` is generated by a transport component and never by a Provider; all others come from the Provider.

| Code | Meaning |
|---|---|
| `INVALID_ARGUMENT` | Missing required parameter, malformed field, value outside its type's syntax or range, invalid binary encoding, or a cursor used outside its context |
| `PAYLOAD_TOO_LARGE` | A PSBT, raw transaction, message or list exceeds an advertised limit of Core §4.1 |
| `UNSUPPORTED_METHOD` | Method not implemented by this Provider |
| `UNSUPPORTED_OPERATION` | Method exists but is not supported for this account, input, format or state |
| `UNSUPPORTED_PSBT_VERSION` | PSBT is not version 0 |
| `UNSUPPORTED_SCRIPT` | Script type or sighash mode not supported |
| `UNAUTHENTICATED` | No valid authenticated session/principal |
| `PERMISSION_REQUIRED` | No active grant of this Client holds the required scope |
| `PERMISSION_REVOKED` | No active grant holds the required scope, but a retained revoked or expired grant of this Client did |
| `GRANT_NOT_FOUND` | No such grant belonging to this Client |
| `GRANT_CONSTRAINT_VIOLATED` | Request exceeds a constraint of the grant, such as `max_amount_sats_per_request` |
| `USER_REJECTED` | User denied request |
| `WALLET_LOCKED` | Wallet is locked; its data cannot be read until the User unlocks it |
| `WALLET_NOT_FOUND` | No such wallet visible to this Client (nonexistent or outside its grants) |
| `ACCOUNT_NOT_FOUND` | No such account visible to this Client (nonexistent or outside its grants) |
| `NETWORK_MISMATCH` | An address is encoded for a network other than the selected account's, or than the stated `network` |
| `NETWORK_UNAVAILABLE` | Selected network/backend unavailable |
| `SYNC_INCOMPLETE` | Wallet has not completed the synchronization the action needs |
| `STATE_UNAVAILABLE` | Required balance, fee, UTXO or confirmation data cannot be made coherent |
| `SIGNER_UNAVAILABLE` | Required hardware/external signer unavailable |
| `INSUFFICIENT_FUNDS` | Insufficient eligible value after fees/policy |
| `POLICY_VIOLATION` | Wallet/user risk policy denies action |
| `PSBT_VALIDATION_FAILED` | Invalid, inconsistent or unreviewable PSBT |
| `PSBT_CONFLICT` | PSBTs of different transactions, or conflicting values for the same key, during combination |
| `TRANSACTION_REJECTED` | Broadcast backend refused transaction |
| `TRANSACTION_NOT_FOUND` | The account has no record of the requested transaction |
| `RESERVATION_NOT_FOUND` | No such reservation of this Client for this account |
| `OPERATION_NOT_FOUND` | Unknown, no longer retained or unauthorized operation reference |
| `OPERATION_NOT_CANCELABLE` | Operation already past an irreversible step |
| `OPERATION_EXPIRED` | Approval or operation window expired |
| `OPERATION_CANCELED` | Operation was canceled before an irreversible step |
| `IDEMPOTENCY_CONFLICT` | Same key used with different parameters |
| `CURSOR_EXPIRED` | Listing cursor cannot be continued safely |
| `RATE_LIMITED` | Client exceeds a request quota or `max_concurrent_operations` |
| `PROVIDER_UNAVAILABLE` | The Provider cannot currently be reached. Generated by a transport component, such as a browser bridge or a relay client, never by the Provider itself |
| `INTERNAL_ERROR` | Unexpected Provider error; without sensitive diagnostics |

## 7. Event types

| Event type | Envelope fields | Payload |
|---|---|---|
| `wallet.locked` | none | `{ wallet_id: Id }` |
| `wallet.unlocked` | none | `{ wallet_id: Id }` |
| `wallet.syncChanged` | none | `{ wallet_id: Id, sync: SyncStatus }` |
| `account.balanceChanged` | `account_id` | `{}` |
| `account.transactionChanged` | `account_id` | `{ txid: Hash256, status: string }`, `status` as in `TransactionRecord` |
| `operation.stateChanged` | `operation_id` | `{ state: string }`, `state` as in `Operation` |
| `grant.revoked` | none | `{ grant_id: Id }` |
| `grant.expired` | none | `{ grant_id: Id }` |
| `session.expired` | none | `{ session_id: Id }` |

## 8. Profiles and protocols

| Profile | Status | Defined in |
|---|---|---|
| `pwc-core-read-v1` | Mandatory | PWC-CORE-001 v0.2.1 §5.1 |
| `pwc-psbt-signer-v1` | Optional | PWC-CORE-001 v0.2.1 §5.2 |

| Protocol identifier | Defined in | Use |
|---|---|---|
| `pwc-local/1` | PWC-TRANSPORT-LOCAL-001 v0.1.0 | Value of `local.hello` `protocols` and `protocol` |
| `pwc-browser/1` | PWC-TRANSPORT-BROWSER-001 v0.1.0 | Value of `PwcProviderInfo.protocol` |
| `pwc-remote/1` | PWC-TRANSPORT-REMOTE-001 v0.1.0; used unchanged by PWC-TRANSPORT-MOBILE-001 v0.1.0 | Version `v=1` of the `pwc-remote:` pairing URI |

## 9. Networks, script types and message formats

| Network | Meaning |
|---|---|
| `mainnet` | Existing Purity main network |
| `signet` | Existing Purity signet |
| `regtest` | Existing Purity regression-test network |

Script types (Core §3.2): `p2pkh`, `p2sh`, `p2sh-p2wpkh`, `p2sh-p2wsh`, `p2wpkh`, `p2wsh`, `p2tr`, `other`.

Message formats (Core §3.2, §8.4): `bip322-simple`, `bip322-full`, `legacy`.

## 10. Transport identifiers

| Identifier | Use | Defined in |
|---|---|---|
| JSON-RPC error code `1000` | Carries every `PwcError` in `error.data` | PWC-TRANSPORT-LOCAL-001 v0.1.0 §4.3 |
| JSON-RPC error codes `-32700`, `-32600` | Parse error; invalid request | PWC-TRANSPORT-LOCAL-001 v0.1.0 §4.3 |
| Endpoint directory `pwc` | Unix socket directory name under the platform base path | PWC-TRANSPORT-LOCAL-001 v0.1.0 §3.2 |
| Pipe prefix `pwc-<sid>-` | Windows named pipe prefix | PWC-TRANSPORT-LOCAL-001 v0.1.0 §3.3 |
| HMAC labels `PWC-LOCAL-1 client`, `PWC-LOCAL-1 provider` | Authentication proofs | PWC-TRANSPORT-LOCAL-001 v0.1.0 §5.4 |
| DOM events `pwc:requestProvider`, `pwc:announceProvider` | Provider discovery | PWC-TRANSPORT-BROWSER-001 v0.1.0 §4.1 |
| Event type `pwcevent` | Core events on the page interface | PWC-TRANSPORT-BROWSER-001 v0.1.0 §4.3 |
| Error name `PwcError` | `name` of a rejected request's `Error` | PWC-TRANSPORT-BROWSER-001 v0.1.0 §4.2 |
| URI scheme `pwc-remote` | Pairing URI | PWC-TRANSPORT-REMOTE-001 v0.1.0 §4.1 |
| Nostr event kind `27180` | PWC messages (ephemeral; registration pending) | PWC-TRANSPORT-REMOTE-001 v0.1.0 §5.1 |
| Hash label `PWC-REMOTE-1 sas` | Short authentication string | PWC-TRANSPORT-REMOTE-001 v0.1.0 §4.4 |
| Link parameter `pwc_session` | Session reference in wake and return links | PWC-TRANSPORT-MOBILE-001 v0.1.0 §3 |

## 11. Provider identifiers

| `provider_id` | Holder |
|---|---|
| `puritywallet` | PurityWallet desktop and its future mobile and extension editions |

## 12. References

- PWC-CORE-001 v0.2.1
- PWC-TRANSPORT-LOCAL-001 v0.1.0
- PWC-TRANSPORT-BROWSER-001 v0.1.0
- PWC-TRANSPORT-REMOTE-001 v0.1.0
- PWC-TRANSPORT-MOBILE-001 v0.1.0
- PWC-GOVERNANCE-001 v0.1.0

---

**End of PWC-REGISTRY-001 v0.1.0.**
