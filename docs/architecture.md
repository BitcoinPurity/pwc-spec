# Architecture

# PWC LOCAL Client (Rust)

## 1. Modules

```text
pwc-client/
├── Cargo.toml
├── src/
│   ├── lib.rs          module declarations and re-exports
│   ├── error.rs        ErrorCode, PwcError, Error
│   ├── types.rs        Core wire types, method parameters and results
│   ├── local.rs        LOCAL transport: endpoint directory, discovery, frames, JSON-RPC, handshake
│   ├── client.rs       Client: raw request, typed Core methods, helpers
│   └── credential.rs   Credential, CredentialStore, FileCredentialStore
└── tests/              integration tests over Unix sockets
```

## 2. Layers

```text
Application / future language SDK
   │  typed methods, helpers            CredentialStore (implemented by the host)
   ▼
client.rs ──request(method, params)──► local.rs (Connection) ──frames──► Provider
```

- **local.rs** owns the transport: the directory check, the socket, frames, the JSON-RPC envelope, `local.hello`, `local.pair`, `local.authenticate` and the proofs.
- **client.rs** owns everything above `request`: serialization of parameters, parsing of results, and the helpers. It does not know about sockets.

There is no transport trait: LOCAL is the only transport. One is introduced when a second transport is added.

## 3. Connection

- A connection has one reader task. It reads frames, hands each response to the waiting request through a oneshot channel keyed by `id`, and sends each `pwc.event` notification into a broadcast channel of capacity 256.
- Writes go through a mutex around the socket's write half, one frame at a time.
- When the socket closes or a frame cannot be parsed, the reader task ends and every waiting request gets `ConnectionClosed`.
- `id` values are integers counted per connection, so an outstanding `id` is never reused.

## 4. Foreign-function readiness

The public interface keeps to these rules so that a later binding crate needs no adapter logic:

| Rule | Reason |
|---|---|
| `Client` is a cheap-to-clone handle; every method takes `&self` | Maps to a foreign object |
| No lifetimes, generics or borrowed return values in public signatures | Binding generators cannot express them |
| Data types are records and enumerations with public fields | Map to foreign records |
| One `Error` enumeration | Maps to one foreign exception type |
| Events are pulled with `EventStream::next` | No callback crosses the boundary |
| `CredentialStore` is the only interface the host implements | Maps to a foreign trait backed by the platform's credential store |

`serde_json::Value` appears in `request`, `Operation::result` and `PwcEvent::payload`; a binding passes it as JSON text.

## 5. Types

- Amounts and fee rates are integers in Rust (`u64`, `i64`) and canonical decimal strings on the wire. Parsing rejects non-canonical forms and values above 2100000000000000 (fee rates: 1000000000).
- Timestamps, hashes, hex, base64, addresses, identifiers and cursors stay strings in their wire form.
- Results accept unknown members (Core §3.1). Optional members are omitted, never `null`.
- Error codes outside Core §10 parse as `ErrorCode::Other`.

## 6. Dependencies

| Crate | Use |
|---|---|
| tokio | Unix sockets, tasks, channels, timers |
| serde, serde_json | Wire format |
| hmac, sha2 | Authentication proofs, compared in constant time |
| getrandom | Nonces, pairing codes, idempotency keys |
| libc | `geteuid` |
| jsonschema, tempfile (tests only) | Validating frames against `pwc-schema/`; temporary endpoint directories |
