# Purity Wallet Connect (PWC)
## Transport Profile LOCAL: Same-Computer Native Clients

**Specification:** PWC-TRANSPORT-LOCAL-001  
**Version:** 0.1.0 (review draft; not a deployed standard)  
**Date:** 2026-11-21  
**Language:** English (normative draft)  
**License:** CC BY 4.0  
**Requires:** PWC-CORE-001 v0.2.1  
**Status:** Proposed specification, subject to owner review and security audit

> **Scope rule:** This profile defines how a native Client process reaches a Provider running on the **same computer under the same operating-system user**: where the endpoint lives, how messages are framed, how a Client is paired and authenticated, and how Core methods, errors and events map onto the connection. It changes no Core semantics. Web pages and browser extensions are covered by PWC-TRANSPORT-BROWSER, other devices by PWC-TRANSPORT-REMOTE.

## 0. Decisions, status and interpretation

### 0.1 Decisions adopted in v0.1.0

- The endpoint is an operating-system local socket: an `AF_UNIX` stream socket on macOS and Linux, a named pipe on Windows. The LOCAL endpoint is never exposed on a network socket, loopback TCP included.
- Messages are JSON-RPC 2.0 objects, one per line.
- A Client is paired once through a User prompt and receives a `client_id` and a 256-bit `client_secret`. Each later connection authenticates both sides with HMAC-SHA256 over fresh nonces.
- One authenticated connection is one Core session. Grants and Operations belong to the `client_id` and survive reconnection.
- Every Core error travels as a JSON-RPC error with code `1000` and the `PwcError` in `data`.
- Events are pushed on the connection as `pwc.event` notifications, without replay.
- A Client paired as a *delegator*, such as a browser bridge, can open delegated principals that act for websites, each with its own `client_id` and grants (§5.8).

The present version remains a **review draft**. Open questions are listed in Appendix A.

### 0.2 Normative vocabulary

Requirement keywords have the meanings given in PWC-CORE-001 §0.4. Terms such as Provider, Client, Session, Grant and Operation, and the semantic types `Id`, `Hex` and `Text`, are those of PWC-CORE-001. References of the form "Core §n" point into PWC-CORE-001 v0.2.1.

## 1. Scope

### 1.1 Covered

A *LOCAL Client* is a native process (a desktop application, a command-line tool, a service) that runs on the same computer as the Provider and under the same operating-system user account.

### 1.2 Not covered

- Web pages and browser extensions. A browser cannot open the endpoints of §3, and a native bridge process that relays requests from web content MUST NOT forward them under its own LOCAL identity; it uses the delegated principals of §5.8, as PWC-TRANSPORT-BROWSER requires.
- Processes running under another operating-system user, or on another computer.
- Sandboxed applications that the operating system prevents from reaching the endpoint paths of §3 (Appendix A, LA-05).

A Provider MUST NOT accept LOCAL connections on any network socket, including loopback TCP and loopback UDP.

## 2. Threat model

| Attacker | Protected | Mechanism |
|---|---|---|
| Remote network attacker | Yes | No network listener (§1.2) |
| Web page in a local browser | Yes | Browsers cannot open local sockets or named pipes |
| Another operating-system user on the same computer | Yes | Endpoint access control (§3) and peer user check (§3.4); a fake endpoint created by that user fails mutual authentication (§5.4) |
| Passive observer of logs or crash dumps | Yes | The secret never crosses the connection after pairing; proofs are bound to fresh nonces (§5.4); secrets are never logged (§7) |
| Malicious code running as the **same** operating-system user | **No** | Such code can read a Client's stored secret, impersonate a Client and imitate its window. Core's per-action User approval (Core §9.2) is then the only remaining protection |
| Compromised operating system or administrator | No | Out of scope |

Because same-user code is not isolated, a LOCAL session can never do more without a User prompt than the Client's grants allow (Core §6). This profile adds no unattended authority.

## 3. Endpoints

### 3.1 Provider identifier

Each Provider has a `provider_id` matching `[a-z0-9-]{1,32}`, for example `puritywallet`. A Provider MUST use the same `provider_id` across releases. Two Providers installed on one computer MUST NOT share a `provider_id`.

### 3.2 macOS and Linux

The endpoint is an `AF_UNIX` socket of type `SOCK_STREAM` named `<provider_id>.sock` in the *endpoint directory*:

| Platform | Endpoint directory |
|---|---|
| Linux and other Unix-like systems | `$XDG_RUNTIME_DIR/pwc` if `XDG_RUNTIME_DIR` is set to an absolute path owned by the user; otherwise `$HOME/.pwc/run` |
| macOS | `$HOME/Library/Application Support/PWC/run` |

The following rules apply to the Provider:

- It MUST create the endpoint directory with mode `0700` if it does not exist. If the directory exists, it MUST be owned by the user and grant no permission to group or others; otherwise the Provider MUST NOT listen and MUST report the problem to the User.
- It MUST create the socket file with mode `0600`.
- If the full socket path is longer than the operating system allows (104 bytes on macOS and 108 bytes on Linux, including the terminating NUL), the Provider MUST NOT fall back to another location; it reports the problem to the User.
- If a socket file already exists, the Provider attempts to connect to it. If the connection is refused, the file is stale and the Provider replaces it. If the connection succeeds, another Provider instance is running, and the Provider MUST NOT replace the file.
- It SHOULD remove its socket file when it stops listening.

A Client MUST apply the same ownership and permission checks to the endpoint directory before connecting, and MUST NOT connect if they fail.

### 3.3 Windows

The endpoint is a named pipe:

```text
\\.\pipe\pwc-<sid>-<provider_id>
```

`<sid>` is the string form of the user's security identifier, for example `S-1-5-21-1004336348-1177238915-682003330-1001`. The pipe namespace is shared by all users of the computer, so both sides take these precautions:

- The Provider MUST create the first pipe instance with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS` and byte mode, and with a security descriptor whose DACL grants access to the user's SID only.
- If creating the first instance fails because the name already exists and no instance of the same Provider is running, the Provider MUST NOT use the pipe and MUST report the problem to the User.
- Before sending its first message on a connection, a Client MUST verify that the pipe server process runs under the Client's own SID, using `GetNamedPipeServerProcessId` and the token of that process. If the check fails or cannot be made, the Client MUST disconnect.

### 3.4 Peer user check

On every accepted connection the Provider MUST verify that the peer process runs under the Provider's own operating-system user: through `SO_PEERCRED` or `getpeereid` on Unix-like systems, and through `GetNamedPipeClientProcessId` and the process token on Windows. If the peer runs under another user, or the user cannot be determined, the Provider MUST close the connection without sending any message.

Where the operating system reveals them, the Provider SHOULD also record the peer's process ID, executable path and code-signing identity, for display under §6.

### 3.5 Discovery

A Client discovers Providers by listing `*.sock` files in the endpoint directory on macOS and Linux, and by enumerating pipe names that begin with `pwc-<sid>-` on Windows. It learns a Provider's name and version through `local.hello` (§5.2). If several Providers are found, the Client SHOULD let the User choose one.

Discovery and `local.hello` reveal only that a Provider is running and which software it is. They MUST NOT reveal wallet inventory, addresses or balances (Core §7, step 1).

## 4. Framing and message format

### 4.1 Frames

A connection carries a stream of frames in each direction. A frame is one compact JSON text encoded in UTF-8 without a byte-order mark, followed by a single line feed (`0x0A`). A frame MUST NOT contain `0x0A` anywhere except as its terminator; standard JSON escaping guarantees this for strings, so compact serialization is sufficient.

`max_frame_bytes`, announced in `local.hello`, is the largest frame either side may send, terminator included. It is at least 1,048,576. A Provider MUST choose it large enough for every request its advertised `Capabilities.limits` permit, for example `psbt.combine` with `limits.max_combine_psbts` PSBTs of `limits.max_psbt_bytes` each in base64. When a Provider receives a longer frame it MUST send the error of §4.3 with `id` `null` and the Core code `PAYLOAD_TOO_LARGE`, and then close the connection.

### 4.2 JSON-RPC 2.0 mapping

Each frame holds one JSON-RPC 2.0 object:

- **Request (Client to Provider).** `jsonrpc` is `"2.0"`; `method` is a registered method name; `params` is an object and is REQUIRED, `{}` for a method without parameters; `id` is a string of 1 to 64 characters or an integer within the safe-integer range of Core §3.2.
- **Response (Provider to Client).** Either `result`, holding the Core result object, or `error` (§4.3), with the `id` of the request.
- **Notification (Provider to Client).** Only `pwc.event` (§8). Clients MUST NOT send notifications; a Provider discards them without a reply.

The following rules apply:

- JSON-RPC batches are not supported. A frame holding an array receives a `-32600` error.
- A Client MUST NOT reuse the `id` of a request that is still outstanding on the same connection. A request with such an `id` receives a `-32600` error and is not processed.
- A Provider MAY answer outstanding requests in any order. Every Core method returns promptly, since methods that wait on the User return an Operation (Core §9); only `local.pair` (§5.3) waits on the User before responding.
- Core §3.1 governs the content of `params` and `result`, including the treatment of unknown members.

### 4.3 Errors

| Situation | `error.code` | `error.message` | `error.data` |
|---|---|---|---|
| Frame is not valid JSON | `-32700` | `"Parse error"` | omitted |
| Valid JSON but not a single JSON-RPC 2.0 request object with an object `params`, or a duplicate outstanding `id` | `-32600` | `"Invalid Request"` | omitted |
| Any Core or LOCAL error, including an unknown method (`UNSUPPORTED_METHOD`) and invalid parameters (`INVALID_ARGUMENT`) | `1000` | The `PwcError.message` | The complete `PwcError` (Core §10) |

The standard JSON-RPC codes `-32601`, `-32602` and `-32603` are not used: those situations are Core errors and are evaluated in the order of Core §8. After a `-32700` or `-32600` error the connection remains usable, because framing is unaffected. A Client MUST branch on `error.data.code`, never on `error.message`.

## 5. Connection lifecycle

### 5.1 Overview

```text
connect ──► local.hello ──┬─► local.pair ─────────┬─► authenticated session ──► close
                          └─► local.authenticate ─┘
```

Before authentication, a connection accepts only the `local.*` methods of this section; any other method fails with `UNAUTHENTICATED`. Time limits apply:

- `local.hello` MUST arrive within 10 seconds of connecting.
- `local.pair` or `local.authenticate` MUST arrive within 60 seconds of `local.hello`.
- A Provider closes a connection that misses either limit.

The `local.*` method names are registered by this profile under the PWC family namespace; Core §3.1 naming rules apply to them.

### 5.2 `local.hello`

`local.hello` MUST be the first request on every connection. If any other request arrives first, the Provider answers it with `UNAUTHENTICATED` and closes the connection.

```typescript
type LocalHelloParams = { protocols: string[] };      // e.g. ["pwc-local/1"]
type LocalHelloResult = {
  protocol: string;                                   // the chosen protocol, "pwc-local/1"
  pwc_version: string;                                // Core version, e.g. "0.2.1"
  provider_id: string;
  provider: { name: Text; version: Text };
  max_frame_bytes: PositiveInteger;
  server_nonce: Hex;                                  // 32 random bytes
  delegation: boolean;                                // true if §5.8 is implemented
};
```

This document defines the protocol `pwc-local/1`. If the Client lists no protocol the Provider supports, the Provider answers with `UNSUPPORTED_OPERATION` and closes the connection. A Provider MUST generate `server_nonce` with a cryptographically secure random generator and MUST NOT reuse it. A connection accepts `local.hello` only once.

### 5.3 Pairing: `local.pair`

A Client without a stored credential pairs:

```typescript
type LocalPairParams = {
  client_name: Text;                // claimed application name, untrusted (§6)
  client_description?: Text;
  pairing_code: string;             // 6 decimal digits
  delegator?: boolean;              // request delegator pairing, §5.8; omitted: false
};
type LocalPairResult = {
  client_id: Id;
  client_secret: Hex;               // 32 random bytes, i.e. 64 hex characters
  session_id: Id;
};
```

The Client generates `pairing_code` from a cryptographically secure random generator and displays it to the User. The Provider then shows a pairing prompt containing:

- the claimed `client_name`, labeled as unverified;
- the peer information recorded under §3.4;
- the `pairing_code`.

The User compares the code with the one the Client displays and approves or refuses the prompt. The response is sent when the User decides:

- **Approved:** the Provider creates a new `client_id` and a new `client_secret` from a cryptographically secure random generator and stores them (§7). The connection becomes an authenticated session of that `client_id`.
- **Refused:** the request fails with `USER_REJECTED`.
- **No answer within 300 seconds:** the request fails with `OPERATION_EXPIRED`. A Provider MAY use a shorter limit.

If the connection closes while the prompt is open, the Provider MUST withdraw the prompt and create no credential.

A request with `delegator: true` follows §5.8. Pairing grants no scope. After pairing, the Client requests scopes with `session.requestPermissions` (Core §8.1). Pairing prompts are approval prompts under Core §12, item 13: a Provider MUST rate-limit them per peer and MUST NOT show more than one pairing prompt per connection. Requests over the limit fail with `RATE_LIMITED`.

### 5.4 Authentication: `local.authenticate`

A paired Client authenticates each new connection:

```typescript
type LocalAuthenticateParams = {
  client_id: Id;
  client_nonce: Hex;                // 32 random bytes
  client_proof: Hex;                // 32 bytes
};
type LocalAuthenticateResult = {
  session_id: Id;
  server_proof: Hex;                // 32 bytes
};
```

With `K` the 32 bytes of `client_secret`, `SN` the 32 bytes of `server_nonce`, `CN` the 32 bytes of `client_nonce`, `ID` the UTF-8 bytes of `client_id`, and `‖` concatenation:

```text
client_proof = HMAC-SHA256(K, "PWC-LOCAL-1 client"   ‖ 0x00 ‖ SN ‖ CN ‖ ID)
server_proof = HMAC-SHA256(K, "PWC-LOCAL-1 provider" ‖ 0x00 ‖ CN ‖ SN ‖ ID)
```

The labels are ASCII. Every field before `ID` has a fixed length, so the input is unambiguous. The two sides proceed as follows:

1. The Client generates `client_nonce` with a cryptographically secure random generator, computes `client_proof` and sends `local.authenticate`.
2. The Provider looks up `client_id` and compares `client_proof` in constant time. An unknown `client_id` and a wrong proof MUST produce the same `UNAUTHENTICATED` error with the same content, after which the Provider closes the connection.
3. On success the Provider returns `server_proof`, and the connection becomes an authenticated session.
4. The Client MUST compare `server_proof` in constant time before sending any other request. On mismatch it MUST close the connection and MUST NOT send further data on it.

A connection allows one `local.authenticate` attempt. A Provider MUST rate-limit failed attempts per peer process and SHOULD delay its response to a failed attempt.

*Informative:* the Client proves first. A fake endpoint learns only a proof bound to its own `server_nonce`, which the real Provider never accepts. A real-time relay would require the attacker to reach the real endpoint, which §3 prevents for other operating-system users.

### 5.5 Sessions

| Core concept | LOCAL binding |
|---|---|
| Authenticated principal | The holder of `client_secret` for a `client_id` |
| `client_id` | Assigned by the Provider at pairing |
| Session | One authenticated connection |
| `session_id` | Assigned at pairing or authentication; ends when the connection closes |
| `SessionInfo.created_at` | Time of pairing or authentication |
| `SessionInfo.grant_ids` | The Client's active grants |

A Client MAY hold several connections at once; each is its own session. A Provider MAY limit connections per `client_id` and refuses further authentications with `RATE_LIMITED`. Grants and Operations belong to the `client_id`, not to a session, so they survive reconnection (Core §7, step 7).

A Provider MAY close an idle session. Before closing a session for any reason other than the Client closing it, the Provider SHOULD send a `session.expired` event on it.

### 5.6 Closing and recovery

Either side may close a connection at any time. Responses to requests still outstanding on a closed connection are lost. A Core method that returns an Operation may nevertheless have created one, so a Client that lost a response reconnects and retries with the same `idempotency_key` (Core §9.3), which returns the original Operation, or calls `operation.get`.

### 5.7 Unpairing: `local.unpair`

An authenticated Client can end its pairing:

```typescript
type LocalUnpairParams = {};
type LocalUnpairResult = { unpaired: true };
```

The Provider deletes the Client's credential and revokes all of its grants with the effects of Core §7, step 6: pending Operations fail with `PERMISSION_REVOKED`, and `grant.revoked` events are delivered where applicable. After sending the response, the Provider closes every connection of that `client_id`.

`local.unpair` carries no `idempotency_key`, an exemption from Core §9.3. Once unpaired, the `client_id` can no longer authenticate, so a retry fails with `UNAUTHENTICATED` and cannot repeat the effect.

The Provider MUST let the User list paired Clients and unpair any of them. The list shows the claimed name, the pairing date, the last connection time and the recorded peer information. Unpairing from the Provider's interface has the same effect as `local.unpair`.

### 5.8 Delegated principals (optional)

A *delegator* is a paired Client that relays requests on behalf of other principals: in this version, websites reached through a browser bridge (PWC-TRANSPORT-BROWSER §7). A Provider that implements this section announces `delegation: true` in `local.hello`.

**Delegator pairing.** A Client requests it with `delegator: true` in `local.pair`. The pairing prompt MUST state that the application will act on behalf of websites, that the Provider will trust the website identities it reports, and that each website still needs its own permission. A Provider that does not implement this section, or whose User declines delegator pairing, fails the request with `UNSUPPORTED_OPERATION` or `USER_REJECTED` respectively.

**Delegated principals.** On an authenticated connection of a delegator, the following methods are available; on any other connection they fail with `UNSUPPORTED_OPERATION`.

```typescript
type DelegatePrincipal = {
  kind: "web_origin";
  origin: string;                   // serialized origin, e.g. "https://app.example"
  private: boolean;                 // true for a private-browsing context
};
type LocalOpenDelegateParams = { principal: DelegatePrincipal };
type LocalOpenDelegateResult = { delegate_id: Id; client_id: Id; session_id: Id };
type LocalDelegatedParams = { delegate_id: Id; method: string; params: object };
type LocalCloseDelegateParams = { delegate_id: Id };
type LocalCloseDelegateResult = { closed: true };
type LocalDelegatedEvent = { delegate_id: Id; event: PwcEvent };   // notification params
```

The following rules apply:

- `local.openDelegate` opens a delegated session for one principal. For `private: false` the Provider maps the pair (delegator `client_id`, `origin`) to a stable delegated `client_id`, created on first use. For `private: true` it creates a new `client_id` that is deleted, with its grants, when the delegated session ends.
- `origin` MUST be the serialization of an `https` origin, or of an `http` origin whose host is `localhost` or a loopback address; anything else fails with `INVALID_ARGUMENT`.
- `local.delegated` performs one Core method as the delegated principal. Its response carries the inner result, or the inner `PwcError` as in §4.3. The inner request is evaluated with the order of checks of Core §8 for the delegated `client_id`, and its grants, Operations, idempotency records and rate limits belong to that `client_id`. An inner `method` beginning with `local.` fails with `INVALID_ARGUMENT`.
- Events for a delegated session are sent as `local.delegatedEvent` notifications, under the rules of §8 applied to the delegated `client_id`.
- `local.closeDelegate` ends one delegated session. Closing the connection ends all delegated sessions opened on it.
- A Provider MAY limit delegated sessions per connection and refuses further ones with `RATE_LIMITED`.
- Every prompt for a delegated principal MUST show its `origin` as the requester and the delegator's claimed name and peer information (§6) as the path through which the request came.
- The Provider's list of paired Clients (§5.7) MUST show each delegator's delegated origins and let the User revoke them one by one. Unpairing a delegator deletes all of its delegated principals and their grants.

*Informative:* a delegator can claim any origin, so delegator pairing is a high-trust decision. This is why the pairing prompt has to say so explicitly.

## 6. Client identity and display

`client_name` and `client_description` are claims made by the Client and are untrusted display text (Core §2, §3.5). The following rules apply:

- Every prompt that shows them MUST mark them as claimed by the application and MUST NOT present them as verified.
- A Provider SHOULD show, next to them, the executable path and code-signing identity recorded under §3.4, labeled as reported by the operating system.
- A Provider MAY record the executable identity at pairing and warn the User at a later authentication if it has changed. Software updates change executables, so such a change MUST NOT by itself fail authentication.

## 7. Credential storage

**Client.** A Client MUST store `client_secret` where other operating-system users cannot read it, and SHOULD use the operating system's credential store: Keychain on macOS, Credential Manager or DPAPI on Windows, Secret Service on Linux. It MUST NOT log the secret, pass it on a command line or in an environment variable, or send it anywhere.

**Provider.** The Provider needs the secret itself to verify proofs, so it cannot store only a hash. It MUST store each `client_secret` with protection no weaker than that of its own wallet files, MUST NOT log it, and MUST delete it when the Client is unpaired.

Secret rotation is not defined in this version (Appendix A, LA-02).

## 8. Events

A Provider that implements this section MAY advertise the Core feature `event_push`. The following rules apply:

- Events are sent as JSON-RPC notifications `{"jsonrpc":"2.0","method":"pwc.event","params":<PwcEvent>}` on every authenticated connection of a Client that holds the scope `events:read`, under the disclosure rules of Core §8.6.
- On one connection, events are sent in the order they occurred. Events that occur while no connection is open are not replayed; after reconnecting, a Client re-queries authoritative state (Core §8.6).
- A Provider MUST NOT silently drop an event for a connection. If a Client does not read fast enough and the Provider's output buffer for that connection exceeds its bound, the Provider closes the connection; the Client then reconnects and re-queries.
- A slow Client MUST NOT delay delivery to other Clients.

A Client without `events:read` learns of state changes by polling `operation.get` and the read methods. It SHOULD NOT poll any single resource more often than once per second.

## 9. Mapping of Core obligations

| Core requirement | Where this profile meets it |
|---|---|
| Authenticate the Client principal and bind it to the grant (Core §2) | §5.3, §5.4, §5.5 |
| Discovery discloses no wallet data (Core §7, step 1) | §3.5, §5.2 |
| Establish `client_id`, verify credentials, bind to the channel (Core §7, step 2) | §5.3, §5.4 |
| Re-authentication and recovery (Core §7, step 7) | §5.4, §5.6 |
| Outer request/response envelope (Core §3.1) | §4.2 |
| Error mapping (Core §10, §11) | §4.3 |
| Event delivery (Core §8.6) | §8 |
| Authenticated, confidential, replay-resistant binding (Core §12, item 9) | Authenticated: §5.3, §5.4. Confidential: endpoint access control and peer check, §3. Replay-resistant: fresh nonces, §5.4 |
| Resource limits (Core §12, item 11) | §4.1, §5.1, §5.3, §5.5, §8 |
| Prompt flooding (Core §12, item 13) | §5.3 |
| Web origins as principals behind a native bridge | §5.8 |

## 10. Security considerations

1. **No network exposure.** A LOCAL endpoint on loopback TCP would be reachable by web pages and by every local user; §1.2 forbids it.
2. **Same-user code.** §2 states plainly that same-user malware is not stopped. Providers MUST NOT describe a LOCAL pairing to the User as making a Client trustworthy.
3. **Squatting.** On Unix-like systems the private endpoint directory prevents other users from creating the socket. On Windows, `FILE_FLAG_FIRST_PIPE_INSTANCE`, the Client's server-SID check (§3.3) and mutual authentication (§5.4) together prevent another user's pipe from being accepted.
4. **Logging.** Providers and Clients MUST NOT log secrets, proofs or nonces, and SHOULD NOT log `params` or `result` values that contain PSBTs or account data (Core §12, item 10).
5. **Resource limits.** Frame size (§4.1), handshake time limits (§5.1), pairing prompts (§5.3), connections per Client (§5.5) and event buffers (§8) are all bounded.

## 11. Conformance tests

These tests supplement Core §13 for Providers and Clients claiming this profile.

| ID | Test | Required outcome |
|---|---|---|
| L-01 | Endpoint directory grants group or other permission | Provider does not listen and reports to the User |
| L-02 | Connection from a process of another operating-system user | Closed without any message |
| L-03 | First request is not `local.hello` | `UNAUTHENTICATED`; connection closed |
| L-04 | Core method before authentication | `UNAUTHENTICATED` |
| L-05 | `local.hello` listing only an unknown protocol | `UNSUPPORTED_OPERATION`; connection closed |
| L-06 | Pairing approved | `client_id`, 64-hex-character `client_secret`; connection authenticated |
| L-07 | Pairing refused | `USER_REJECTED`; no credential created |
| L-08 | Pairing unanswered | `OPERATION_EXPIRED` within 300 seconds |
| L-09 | Connection closed while the pairing prompt is open | Prompt withdrawn; no credential created |
| L-10 | Wrong proof versus unknown `client_id` | Identical `UNAUTHENTICATED` responses; connection closed |
| L-11 | `client_proof` replayed from an earlier connection | `UNAUTHENTICATED` |
| L-12 | Test vector of §12.3 | Both proofs match exactly |
| L-13 | Fake endpoint returns a wrong `server_proof` | Client sends nothing further and closes |
| L-14 | Frame longer than `max_frame_bytes` | `PAYLOAD_TOO_LARGE` with `id` `null`; connection closed |
| L-15 | Frame that is not valid JSON | `-32700`; connection remains usable |
| L-16 | Batch request | `-32600` |
| L-17 | Duplicate outstanding `id` | `-32600`; the second request is not processed |
| L-18 | Unknown method after authentication | Code `1000` with `data.code` `UNSUPPORTED_METHOD` |
| L-19 | Core error, e.g. no grant | Code `1000` with `data` equal to the `PwcError` |
| L-20 | Disconnect while `psbt.sign` is pending, then retry with the same key | Same `operation_id` |
| L-21 | Events with and without `events:read` | Delivered only with the scope |
| L-22 | Client stops reading while events accumulate | Connection closed; no event dropped silently |
| L-23 | `local.unpair` | Grants revoked, connections closed, later authentication fails |
| L-24 | Windows pipe name created first by another user | Provider does not use it; Client refuses a server with another SID |
| L-25 | Endpoint path longer than the operating-system limit | Provider reports to the User; no fallback location |
| L-26 | Stale socket file versus running instance | Stale file replaced; running instance's file untouched |
| L-27 | Pairing prompt | Claimed name marked as unverified; operating-system peer information shown where available |
| L-28 | Network interfaces | No LOCAL listener on any TCP or UDP socket |
| L-29 | `local.openDelegate` on a connection of a non-delegator | `UNSUPPORTED_OPERATION` |
| L-30 | Same origin opened twice with `private: false` | Same delegated `client_id` both times |
| L-31 | Delegated session with `private: true` closed | Its `client_id` and grants deleted |
| L-32 | `local.delegated` with an inner `local.*` method | `INVALID_ARGUMENT` |
| L-33 | `local.openDelegate` with a `file:` or plain-`http` remote origin | `INVALID_ARGUMENT` |
| L-34 | Grant made for one delegated origin | Not usable by another origin or by the delegator itself |

## 12. Informative examples

Frames are shown one per line. Values in angle brackets are placeholders.

### 12.1 Pairing

```text
C→P {"jsonrpc":"2.0","id":1,"method":"local.hello","params":{"protocols":["pwc-local/1"]}}
P→C {"jsonrpc":"2.0","id":1,"result":{"protocol":"pwc-local/1","pwc_version":"0.2.1","provider_id":"puritywallet","provider":{"name":"PurityWallet","version":"1.1.1"},"max_frame_bytes":8388608,"server_nonce":"<64 hex>","delegation":true}}
C→P {"jsonrpc":"2.0","id":2,"method":"local.pair","params":{"client_name":"Example Trader","pairing_code":"482913"}}
      … the User compares 482913 in both windows and approves …
P→C {"jsonrpc":"2.0","id":2,"result":{"client_id":"cl-7Qm2xA","client_secret":"<64 hex>","session_id":"ss-91"}}
C→P {"jsonrpc":"2.0","id":3,"method":"session.requestPermissions","params":{"scopes":["wallets:read","balances:read"],"idempotency_key":"grant-001"}}
```

### 12.2 Reconnection and a Core error

```text
C→P {"jsonrpc":"2.0","id":1,"method":"local.hello","params":{"protocols":["pwc-local/1"]}}
P→C {"jsonrpc":"2.0","id":1,"result":{…,"server_nonce":"202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"}}
C→P {"jsonrpc":"2.0","id":2,"method":"local.authenticate","params":{"client_id":"cl-7Qm2xA","client_nonce":"404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f","client_proof":"3c8210fe06538455dd0d56d572cde9134c529c781a5e4e374f2a7cfcaccc1488"}}
P→C {"jsonrpc":"2.0","id":2,"result":{"session_id":"ss-92","server_proof":"a91ace84622f91fef391a014e55570ab722299a9af64aa25f2809c67decc29b2"}}
C→P {"jsonrpc":"2.0","id":3,"method":"utxo.list","params":{"account_id":"acct-01"}}
P→C {"jsonrpc":"2.0","id":3,"error":{"code":1000,"message":"Scope utxos:read is required","data":{"code":"PERMISSION_REQUIRED","message":"Scope utxos:read is required"}}}
```

### 12.3 Authentication test vector

| Input | Value |
|---|---|
| `client_secret` | `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f` |
| `server_nonce` | `202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f` |
| `client_nonce` | `404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f` |
| `client_id` | `cl-7Qm2xA` |
| `client_proof` | `3c8210fe06538455dd0d56d572cde9134c529c781a5e4e374f2a7cfcaccc1488` |
| `server_proof` | `a91ace84622f91fef391a014e55570ab722299a9af64aa25f2809c67decc29b2` |

### 12.4 Event

```text
P→C {"jsonrpc":"2.0","method":"pwc.event","params":{"event_id":"ev-301","event_type":"operation.stateChanged","occurred_at":"2026-11-21T00:02:00Z","operation_id":"op-1001","payload":{"state":"completed"}}}
```

## 13. References

- PWC-CORE-001 v0.2.1, Purity Wallet Connect Core
- JSON-RPC 2.0 Specification: https://www.jsonrpc.org/specification
- RFC 2104, HMAC: https://www.rfc-editor.org/rfc/rfc2104
- FIPS 180-4, Secure Hash Standard (SHA-256): https://csrc.nist.gov/pubs/fips/180-4/upd1/final
- XDG Base Directory Specification: https://specifications.freedesktop.org/basedir-spec/latest/
- Microsoft, `CreateNamedPipeW`: https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew
- Microsoft, `GetNamedPipeServerProcessId`: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid
- Microsoft, `GetNamedPipeClientProcessId`: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientprocessid

## Appendix A. Open questions

| ID | Subject | Current draft policy |
|---|---|---|
| LA-01 | Windows endpoint | Named pipe. `AF_UNIX` is also available on Windows 10 version 1803 and later and would reuse the socket code PurityWallet already has, but Node.js and CPython cannot open `AF_UNIX` sockets on Windows without native extensions |
| LA-02 | Rotation of `client_secret` | Not defined; a Client re-pairs to obtain a new secret |
| LA-03 | Binding a pairing to an executable identity | Recording and warning only (§6); no enforcement |
| LA-04 | Combined pairing and permission prompt | Not defined; pairing and `session.requestPermissions` are separate prompts |
| LA-05 | Sandboxed Clients (macOS App Sandbox, Flatpak, Snap) | Not covered; they may be unable to reach the endpoint directory |
| LA-06 | Delegated principal kinds | Only `web_origin`; other kinds may be added by later profiles |

---

**End of PWC-TRANSPORT-LOCAL-001 v0.1.0 English review draft.**
