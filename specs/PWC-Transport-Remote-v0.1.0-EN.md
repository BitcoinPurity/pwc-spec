# Purity Wallet Connect (PWC)
## Transport Profile REMOTE: Cross-Device Connections over Nostr Relays

**Specification:** PWC-TRANSPORT-REMOTE-001  
**Version:** 0.1.0 (review draft; not a deployed standard)  
**Date:** 2026-11-21  
**Language:** English (normative draft)  
**License:** CC BY 4.0  
**Requires:** PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001 v0.1.0 §4.2 and §4.3 (message format only)  
**Status:** Proposed specification, subject to owner review and security audit

> **Scope rule:** This profile defines how a Client and a Provider on **different devices** exchange PWC messages through Nostr relays: pairing by QR code or link, key handling, encryption, the event format, splitting of large messages, replay protection and availability. It changes no Core semantics. Relays are untrusted carriers.

## 0. Decisions, status and interpretation

### 0.1 Decisions adopted in v0.1.0

- Messages travel as ephemeral Nostr events of kind `27180`, signed with NIP-01 Schnorr signatures and encrypted with NIP-44 version 2.
- Each side uses a fresh secp256k1 *transport key* for each pairing. Transport keys are never wallet keys and never a User's Nostr identity.
- The Client starts pairing with a `pwc-remote:` URI, shown as a QR code or passed as a link. Both devices then display a 6-digit short authentication string (SAS), which the User compares.
- The plaintext of every message is a JSON-RPC 2.0 object with the rules of LOCAL §4.2 and §4.3. Messages larger than 8,192 bytes are split into parts.
- A pairing is one Core session. Grants and Operations belong to the `client_id` assigned at pairing.

The present version remains a **review draft**. Open questions are listed in Appendix A.

### 0.2 Normative vocabulary

Requirement keywords have the meanings given in PWC-CORE-001 §0.4. Core terms and semantic types are used as defined there. "Core §n" points into PWC-CORE-001 v0.2.1, and "LOCAL §n" into PWC-TRANSPORT-LOCAL-001 v0.1.0. "NIP-n" refers to the Nostr Implementation Possibilities in the `nostr-protocol/nips` repository.

## 1. Scope

### 1.1 Covered

A Client and a Provider that run on different devices, for example a web or desktop application with a wallet on a phone, and that can each open WebSocket connections to Nostr relays.

### 1.2 Not covered

- Two applications on the same computer: use LOCAL.
- Web pages reaching an extension: use BROWSER.
- Two applications on the same phone: MOBILE, which builds on this profile.
- Waking a Provider that is not running. Core excludes push services (Core §1.2), so a Provider receives messages only while it is connected to its relays (§8).

## 2. Threat model

| Attacker | Protected | Mechanism |
|---|---|---|
| Relay operator or network observer reading messages | Yes | NIP-44 encryption (§5.3) |
| Relay or third party forging or altering messages | Yes | Signatures by the pinned peer key (§5.4) |
| Replay of a captured event | Yes | Event ids, time window and duplicate suppression (§5.5) |
| Linking a pairing to a User's identity | Partly | Fresh transport keys per pairing (§3). Relays still see timing, approximate sizes and the IP address of each side |
| Someone who photographs the pairing QR code and pairs their own wallet with the Client | Yes, if the User compares the SAS | SAS confirmation on the Client (§4.4) |
| Relay withholding or delaying messages | **No** | Availability is not guaranteed; Core idempotency makes retries safe (§8) |
| Malicious code on either device | **No** | Out of scope; Core per-action approval (Core §9.2) remains |

## 3. Keys

Each side generates a new secp256k1 key pair for each pairing, its *transport key*, and MUST NOT use it for anything else. The Provider learns the Client's transport public key from the pairing URI, and the Client learns the Provider's from the first signed event of the pairing (§4.3). From then on each side accepts events only from the peer key it pinned. Public keys are 32-byte x-only keys as in NIP-01, written as 64 lowercase hex characters.

Each side MUST store its transport private key where other users and applications cannot read it, SHOULD use the platform's secure key storage, and MUST delete it when the pairing ends.

## 4. Pairing

### 4.1 Pairing URI

The Client generates its transport key and a 16-byte random `secret`, and displays:

```text
pwc-remote://<client-pubkey>?v=1&secret=<32 hex>&relay=<url>[&relay=<url>…]&name=<client name>
```

| Parameter | Required | Meaning |
|---|---|---|
| `<client-pubkey>` | Yes | The Client's transport public key, 64 lowercase hex characters |
| `v` | Yes | Protocol version; `1` for `pwc-remote/1` |
| `secret` | Yes | 16 random bytes, 32 lowercase hex characters; single use |
| `relay` | Yes, one or more | `wss:` URL of a relay the Client listens on |
| `name` | Yes | Claimed Client name, a `Text`; untrusted |
| `return` | No | The Client's return URL, used only by PWC-TRANSPORT-MOBILE §5 |

Query values are percent-encoded. The URI is valid for at most 10 minutes; after that the Client stops listening for pairing responses for this `secret`. The Client SHOULD display the URI as a QR code and SHOULD also offer it as a link for MOBILE (PWC-TRANSPORT-MOBILE).

### 4.2 Provider prompt

When the User scans or opens a pairing URI, the Provider generates its own transport key and computes the SAS (§4.4). It then shows a pairing prompt with:

- the claimed `name`, marked as unverified;
- the relays;
- the SAS.

If the User refuses, the Provider MAY send `remote.pairingRejected` (§4.3) and discards the keys.

### 4.3 Pairing messages

On approval, the Provider creates a `client_id` and a `session_id` and sends a `remote.paired` notification to the Client's relays (§5.2):

```typescript
type RemotePairedParams = {
  secret: Hex;                        // echoed from the URI
  client_id: Id;
  session_id: Id;
  pwc_version: string;                // e.g. "0.2.1"
  provider_id: string;                // [a-z0-9-]{1,32}
  provider: { name: Text; version: Text };
  relays: string[];                   // wss: URLs the Provider listens on; at least one
  max_message_bytes: PositiveInteger; // largest reassembled message the Provider accepts; at least 1,048,576
  wake_url?: string;                  // PWC-TRANSPORT-MOBILE §4; present only for a Provider that supports it
};
type RemotePairingRejectedParams = { secret: Hex };
```

The Client accepts the first `remote.paired` whose `secret` matches, and pins the event's author key as the Provider's key. It ignores every other pairing response for that `secret`.

### 4.4 Short authentication string

Both sides compute the SAS from the 32-byte Client key `C`, the 32-byte Provider key `P` and the 16-byte `secret` `S`:

```text
h   = SHA-256("PWC-REMOTE-1 sas" ‖ 0x00 ‖ C ‖ P ‖ S)
SAS = decimal of (first 4 bytes of h, as a big-endian unsigned integer) mod 1,000,000, zero-padded to 6 digits
```

After accepting `remote.paired`, the Client MUST show the SAS and ask the User to confirm that the Provider shows the same digits. Only after confirmation does the Client treat the pairing as established and send requests. If the User reports a mismatch, the Client MUST discard the pairing and SHOULD tell the User to remove it from the wallet.

*Informative:* the `secret` proves that the Provider saw the URI, and the SAS proves that the Provider the User is looking at is the one that answered. Without the SAS, someone who photographed the QR code could answer first and pose as the User's wallet, for example handing out their own receive addresses.

### 4.5 After pairing

The Provider treats the pairing as established once it has sent `remote.paired`. Pairing grants no scope: the Client then calls `session.requestPermissions` (Core §8.1). Pairing prompts are approval prompts under Core §12, item 13.

## 5. Messages

### 5.1 Event format

Every message is a Nostr event as defined in NIP-01, with:

| Field | Value |
|---|---|
| `kind` | `27180`, an ephemeral kind (NIP-01: kinds 20000–29999 are not expected to be stored by relays) |
| `pubkey` | The sender's transport public key |
| `tags` | Exactly one tag `["p", <recipient transport public key>]` |
| `content` | NIP-44 version 2 payload of the message plaintext (§5.3) |
| `created_at` | The sender's current time in seconds |
| `id`, `sig` | As NIP-01 |

### 5.2 Relays

The Client publishes to every relay in the Provider's `relays`, and the Provider publishes to every relay in the Client's pairing URI. Each side subscribes on its own relays with a filter for kind `27180` and its own key in `#p`.

- Each side SHOULD choose relays that accept messages of at least 16,384 bytes (NIP-11 `max_message_length`).
- A relay may require NIP-42 authentication; each side then authenticates with its transport key.
- The Client changes its relays with `remote.updateRelays` (§6). The Provider announces a change of its relays with a `remote.relaysChanged` notification and SHOULD keep listening on the old relays for 10 minutes.

### 5.3 Plaintext and encryption

The plaintext is either a whole message or one part of a message (§5.6). A whole message is one JSON-RPC 2.0 object under LOCAL §4.2 and §4.3, serialized compactly in UTF-8:

- requests from the Client to the Provider;
- responses from the Provider to the Client;
- notifications from the Provider to the Client: `pwc.event` (LOCAL §8 rules apply, with this profile's delivery) and the `remote.*` notifications of this profile.

The plaintext is encrypted with NIP-44 version 2 from the sender's transport key to the recipient's. A plaintext that fails decryption is dropped.

### 5.4 Authentication

A recipient MUST verify each event's `id` and `sig` under NIP-01, and MUST check that `pubkey` is the pinned peer key and that the `p` tag is its own key. Events that fail any check are dropped without reply. During pairing, before the Provider's key is pinned, the Client applies the rules of §4.3 instead.

### 5.5 Replay protection and duplicates

The same event can arrive through several relays, so every recipient suppresses duplicates:

- A recipient MUST drop an event whose `created_at` differs from its own clock by more than 300 seconds.
- It MUST remember the `id` of every accepted event for at least 600 seconds and MUST drop an event whose `id` it has already accepted.
- A JSON-RPC `id` MUST NOT be reused by the Client while the request is outstanding. A Provider that receives a second request with an outstanding `id` from the same Client answers it with `-32600` (LOCAL §4.3).

### 5.6 Splitting large messages

A whole message whose UTF-8 encoding exceeds 8,192 bytes is split. The sender cuts the encoded bytes into slices of at most 6,000 bytes and sends each slice as a separate event whose plaintext is:

```typescript
type RemotePart = { pwc_part: {
  message_id: Hex;     // 16 random bytes, the same for all parts of one message
  index: UInt32;       // 0-based
  count: UInt32;       // number of parts, at least 2
  data: Base64;        // the slice
} };
```

The following rules apply:

- The recipient reassembles a message once it holds every `index` from 0 to `count − 1` for one `message_id` and sender, in any order of arrival.
- It drops all parts of a message that is not complete within 120 seconds of its first part.
- It MUST drop a message whose reassembled size would exceed the limit it announced: the Provider's `max_message_bytes`, or 16,777,216 bytes for a Client.
- It MUST drop a part whose `count` differs from earlier parts of the same message.
- Senders SHOULD pace parts to respect relay rate limits.

## 6. Methods and notifications of this profile

| Name | Kind | Params | Result | Notes |
|---|---|---|---|---|
| `remote.ping` | Request | `{}` | `{}` | Availability check; needs no scope |
| `remote.updateRelays` | Request | `{ relays: string[] }` | `{}` | Replaces the Client's relay list; at least one `wss:` URL |
| `remote.unpair` | Request | `{}` | `{ unpaired: true }` | Ends the pairing; effects as LOCAL §5.7 |
| `remote.paired` | Notification | `RemotePairedParams` | — | §4.3 |
| `remote.pairingRejected` | Notification | `RemotePairingRejectedParams` | — | §4.2 |
| `remote.relaysChanged` | Notification | `{ relays: string[] }` | — | §5.2 |

`remote.unpair` deletes the pairing, revokes all its grants with the effects of Core §7, step 6, and makes the Provider delete the transport keys of that pairing. Like `local.unpair` it carries no `idempotency_key`. The Provider MUST let the User list pairings and remove any of them, with the same effect.

## 7. Sessions

| Core concept | REMOTE binding |
|---|---|
| Authenticated principal | The holder of the Client transport key pinned at pairing |
| `client_id` | Assigned by the Provider in `remote.paired` |
| Session | One pairing |
| `session_id` | Assigned in `remote.paired`; ends when the pairing ends |
| `SessionInfo.created_at` | Time of pairing |

A Provider MAY end pairings that have been unused for a period it chooses. Before ending a pairing for any reason other than `remote.unpair`, it SHOULD send a `session.expired` event if the Client holds `events:read`.

## 8. Availability and retries

A Provider receives requests only while it is connected to its relays. Ephemeral events are not stored, so a request sent while the Provider is offline is lost. The following rules apply:

- A Client SHOULD treat a request that receives no response within 30 seconds as failed, and its library reports `PROVIDER_UNAVAILABLE` to the application. This error is generated by the Client side, never sent by the Provider.
- Before retrying, a Client MAY call `remote.ping`.
- Retries of Core methods MUST reuse the original `idempotency_key` (Core §9.3), so that a request that did arrive is not performed twice.
- A Client SHOULD tell the User to open the wallet when the Provider is unavailable.
- Events are delivered only while both sides are online and are never replayed; on reconnecting, the Client re-queries state (Core §8.6).

## 9. Mapping of Core obligations

| Core requirement | Where this profile meets it |
|---|---|
| Authenticate the Client principal and bind it to the grant (Core §2) | §3, §4.3, §5.4, §7 |
| Discovery discloses no wallet data (Core §7, step 1) | §4: pairing reveals only Provider identity, and only after the User approves |
| Re-authentication and recovery (Core §7, step 7) | Every event is authenticated (§5.4); retries under §8 |
| Outer request/response envelope (Core §3.1) | §5.3 |
| Error mapping (Core §10) | LOCAL §4.3, by reference |
| Event delivery (Core §8.6) | §5.3, §8 |
| Authenticated, confidential, replay-resistant binding (Core §12, item 9) | §5.4, §5.3, §5.5 |
| Relay attacks (Core §12, item 9) | §2, §5.4, §5.5 |
| Prompt flooding (Core §12, item 13) | §4.5 |

## 10. Security considerations

1. **Relays are untrusted.** They can drop, delay, reorder and duplicate events, and they learn transport keys, timing, approximate sizes and IP addresses. They cannot read or forge messages.
2. **Fresh keys.** Reusing a transport key across pairings would let relays link them. A wallet's keys and a User's Nostr identity MUST NOT serve as transport keys.
3. **The SAS matters.** A Client that skips SAS confirmation is exposed to the QR-photographing attack of §2. Clients MUST NOT offer a setting that skips it.
4. **Clock skew.** The 300-second window assumes roughly correct clocks. A device with a badly wrong clock will see its messages dropped rather than accepted late.
5. **Metadata.** Padding in NIP-44 only partly hides sizes. A PSBT split into many parts reveals its approximate size.

## 11. Conformance tests

| ID | Test | Required outcome |
|---|---|---|
| R-01 | Pairing URI with a wrong `secret` echoed | Client ignores the response |
| R-02 | Two `remote.paired` responses for one `secret` | Only the first accepted and pinned |
| R-03 | SAS test vector of §12.2 | `084418` |
| R-04 | User reports a SAS mismatch | Client discards the pairing and sends no request |
| R-05 | Event signed by a key other than the pinned peer | Dropped without reply |
| R-06 | Event with an invalid `sig` or `id` | Dropped |
| R-07 | Event with `created_at` 301 seconds old | Dropped |
| R-08 | Same event delivered by two relays | Processed once |
| R-09 | Message of 20,000 bytes | Sent as 4 parts and reassembled intact, in any arrival order |
| R-10 | Parts of one message not complete within 120 seconds | All parts dropped |
| R-11 | Reassembled message above the announced limit | Dropped |
| R-12 | Request sent while the Provider is offline, retried later with the same key | One Operation only |
| R-13 | No response within 30 seconds | Client library reports `PROVIDER_UNAVAILABLE` |
| R-14 | `remote.unpair` | Grants revoked; later events from the Client dropped |
| R-15 | Transport key reuse across pairings | Never: each pairing has new keys on both sides |
| R-16 | Pairing URI older than 10 minutes | Client no longer accepts a response |
| R-17 | `remote.relaysChanged` | Client publishes to the new relays |
| R-18 | Events with and without `events:read` | `pwc.event` sent only with the scope |

## 12. Informative examples

### 12.1 Pairing URI

```text
pwc-remote://1111111111111111111111111111111111111111111111111111111111111111?v=1&secret=00112233445566778899aabbccddeeff&relay=wss%3A%2F%2Frelay.example.com&name=Example%20Trader
```

### 12.2 SAS test vector

| Input | Value |
|---|---|
| Client key `C` | `1111111111111111111111111111111111111111111111111111111111111111` |
| Provider key `P` | `2222222222222222222222222222222222222222222222222222222222222222` |
| `secret` `S` | `00112233445566778899aabbccddeeff` |
| `h` | `9644b2022d166548aa677e24c6490910c5927f6ae43a53b7209128091d0fb2d2` |
| SAS | `084418` |

The keys above are placeholders for the hash computation and are not valid points on the curve.

### 12.3 Plaintext of a request and its response

```json
{"jsonrpc":"2.0","id":7,"method":"account.getBalance","params":{"account_id":"acct-01"}}
```

```json
{"jsonrpc":"2.0","id":7,"error":{"code":1000,"message":"Scope balances:read is required","data":{"code":"PERMISSION_REQUIRED","message":"Scope balances:read is required"}}}
```

Each is encrypted with NIP-44 and carried in a kind-`27180` event as in §5.1.

## 13. References

- PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001 v0.1.0
- NIP-01, basic protocol and event kinds: https://github.com/nostr-protocol/nips/blob/master/01.md
- NIP-11, relay information document: https://github.com/nostr-protocol/nips/blob/master/11.md
- NIP-42, client authentication to relays: https://github.com/nostr-protocol/nips/blob/master/42.md
- NIP-44, versioned encryption (version 2), including its test vectors: https://github.com/nostr-protocol/nips/blob/master/44.md
- NIP-46, Nostr Connect (informative; the pairing URI of §4.1 follows its `nostrconnect://` pattern): https://github.com/nostr-protocol/nips/blob/master/46.md
- BIP-340, Schnorr signatures for secp256k1: https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki
- FIPS 180-4, SHA-256: https://csrc.nist.gov/pubs/fips/180-4/upd1/final

## Appendix A. Open questions

| ID | Subject | Current draft policy |
|---|---|---|
| RA-01 | Registration of kind `27180` | Unregistered; to be proposed to the NIP kind registry before stabilization |
| RA-02 | Provider-initiated pairing (a `bunker://`-style URI) | Not defined; only the Client starts pairing |
| RA-03 | Offline delivery | Not defined; requests to an offline Provider are lost and retried (§8) |
| RA-04 | Default relays | None defined; each implementation chooses |
| RA-05 | Transport key rotation within a pairing | Not defined; re-pair to change keys |

---

**End of PWC-TRANSPORT-REMOTE-001 v0.1.0 English review draft.**
