# Architecture: pwc-provider and pwc-provider-java

## 1. Modules

```text
pwc-provider/
├── Cargo.toml
├── src/
│   ├── lib.rs          module declarations
│   ├── main.rs         demo binary: arguments, approval modes, startup
│   ├── error.rs        ErrorCode (36 codes) and PwcError
│   ├── types.rs        semantic types (validated when parsed), shared wire types, fee formula
│   ├── methods.rs      method registry and parameter structures
│   ├── backend.rs      WalletBackend trait and its data types
│   ├── memory.rs       MemoryBackend: simulated wallets and chains
│   ├── balance.rs      balance computation (Core §4.3)
│   ├── psbt.rs         raw PSBT key-value parsing, combine, finalize, signing status, size estimates
│   ├── message.rs      BIP-322 simple and legacy message signatures
│   ├── prompt.rs       approval request types
│   ├── provider/
│   │   ├── mod.rs      Provider, Config, state, order of checks, events, save and load
│   │   ├── grants.rs   grant coverage and narrowing
│   │   ├── operations.rs Operation states, idempotency records, reservations
│   │   └── handlers.rs check 10 of each method
│   ├── local.rs        LOCAL transport: frames, JSON-RPC, handshake, generic over the stream
│   └── local/
│       ├── unix.rs     endpoint directory and AF_UNIX socket, peer check through SO_PEERCRED
│       └── windows.rs  named pipe with first-instance flag and DACL, peer check through the process token
└── tests/              integration tests over the Unix socket
```

## 2. Layers

```text
Host wallet / demo binary
   │  Prompt channel (approval)   save_state / with_state   notify(BackendEvent)
   ▼
local.rs ──frames, handshake──► Provider (order of checks, grants, Operations, idempotency, reservations, events)
                                   │
                                   ▼
                             WalletBackend trait ◄── MemoryBackend
```

- **local.rs** handles only the transport: endpoint, peer user, frames, the JSON-RPC envelope, the `local.*` methods and time limits. After authentication it passes Core methods to `Provider::handle`.
- **Provider** is transport-independent and owns the Core semantics: order of checks, grants, the Operation state machine, idempotency, reservations, pagination cursors, event fan-out and the approval flow.
- **WalletBackend** owns wallet data and keys: wallet and account inventory, balances, addresses, UTXOs, history, output ownership, coin selection and PSBT construction, signing, broadcast and message signing.

## 3. Order of checks

`Provider::handle(session, method, params)` follows Core §8 exactly:

| Step | Check | On failure |
|---|---|---|
| 1 | Authenticated (ensured by local.rs) | `UNAUTHENTICATED` |
| 2 | Request quota (not implemented) | — |
| 3 | Method in the registry | `UNSUPPORTED_METHOD` |
| 4 | Parameter syntax; size limits | `INVALID_ARGUMENT`; `PAYLOAD_TOO_LARGE` |
| 5 | Idempotency record: an exact retry gets the original response | `IDEMPOTENCY_CONFLICT` |
| 6 | An active grant holds the scope | `PERMISSION_REVOKED` / `PERMISSION_REQUIRED` |
| 7 | The wallet or account exists and a grant covers it | `WALLET_NOT_FOUND` / `ACCOUNT_NOT_FOUND`, fixed content |
| 8 | Wallet state | `WALLET_LOCKED` / `STATE_UNAVAILABLE` |
| 9 | Unfinished Operations; approval prompt rate | `RATE_LIMITED` |
| 10 | Method-specific checks, including grant constraints | the method's codes |

A successful request writes its idempotency record. A request that fails with a direct error writes none.

## 4. Approval

`Provider::new` returns `(Arc<Provider>, PromptReceiver)`. When the User must decide, the Provider sends a `Prompt` on the channel; each `Prompt` carries a oneshot reply:

| Prompt | Reply |
|---|---|
| `Pairing` | `bool` |
| `Permissions` | `Option<GrantChoice>`; `None` refuses |
| `SignPsbt` | `bool` |
| `SignMessage` | `bool` |

- A host that drops the reply refuses the request.
- The Provider withdraws a prompt by dropping its receiving end (connection closed, canceled, expired, grant revoked). The host can observe this with `reply.is_closed()`.
- The Provider narrows the User's grant choice: scopes are intersected with the request, expiry and constraints are never looser than requested, and resources never exceed those requested when the request listed any.

Each Operation awaiting approval has a task that waits for the first of: the User's reply, expiry, a cancellation signal. After approval the request is authorized again, and the wallet state checked, before anything is signed.

## 5. State and concurrency

- All mutable Provider state lives in one `Mutex<Inner>`, and each request runs under it. Requests are therefore serialized, so requests competing for the same UTXO cannot race (Core §9.3).
- Backend calls are made under the lock. The backend never calls back into the Provider, so this cannot deadlock.
- Each connection has a bounded output channel and a writer task. Responses use `send().await`; events use `try_send`, and a full channel closes that connection (LOCAL §8).
- A pending pairing prompt runs in its own task, which the connection aborts when it closes; aborting drops the reply's receiver and so withdraws the prompt.

## 6. State snapshot

The persistent part of the state serializes to a JSON snapshot:

```text
Snapshot { version: 1, cursor_key, clients, grants, operations, idempotency, reservations }
```

Not included: live sessions, rate-limit counters and waiting tasks. All times are UTC timestamps and compare normally after loading.

## 7. Dependencies

| Crate | Use |
|---|---|
| tokio | Unix sockets, timers, channels, signals |
| serde, serde_json | Wire format and snapshot |
| bitcoin 0.32 | Addresses, transactions, PSBT, sighash, secp256k1, HMAC/SHA-256, legacy message signatures |
| miniscript 13 | `psbt.finalize` only |
| base64 | Canonical padded base64 |
| getrandom | Randomness for secrets, nonces and identifiers |
| time | RFC 3339 timestamps |
| libc | `geteuid` |
| jsonschema, tempfile (tests only) | Validating output against the strict schema; temporary endpoint directories |

## 8. Java Provider SDK

### 8.1 Modules

```text
pwc-provider-java/
├── settings.gradle.kts, build.gradle.kts, gradlew   Gradle 9.1.0
├── native/                    Rust cdylib pwc-provider-jni (path dependency on ../../pwc-provider)
│   └── src/
│       ├── lib.rs             JNI_OnLoad, the shared runtime, error and panic mapping, Config
│       ├── convert.rs         Java values to Rust values and back; the class cache
│       ├── records.rs         backend records in both directions
│       ├── provider.rs        native methods of Provider, LocalServer and Prompt
│       ├── memory.rs          native methods of MemoryBackend
│       ├── backend.rs         JavaBackend: WalletBackend over a Java object
│       └── prompts.rs         the prompt thread and reply handles
└── src/main/java/org/bitcoinpurity/pwc/provider/
    ├── Native.java            every native method; loads the library from the jar
    ├── NativeHandle.java      owns a native pointer; frees it on close or by a Cleaner
    ├── Provider.java, Config.java, Limits.java, RateLimit.java, LocalServer.java
    ├── Prompt.java and its four subclasses, PromptHandler.java, request records
    ├── WalletBackend.java, MemoryBackend.java, backend records and enums, BackendEvent.java
    ├── ErrorCode.java, PwcException.java, StateException.java, EndpointException.java
    └── demo/Main.java         demo program
```

### 8.2 Layers

```text
Java host wallet
   │ Provider, LocalServer, MemoryBackend       ▲ PromptHandler.onPrompt     ▲ WalletBackend calls
   ▼                                            │                            │
Native.java ──JNI──► pwc-provider-jni ──────────┴── prompt thread            JavaBackend
                          │                                                  ▲
                          ▼                                                  │
                     pwc-provider: Provider, local::bind/serve ──────────────┘ (or MemoryBackend directly)
```

- All native methods are static methods of `Native`. Java objects that own native memory hold a `NativeHandle`: a pointer to a `Box` on the Rust side, freed exactly once by `close()` or by a `Cleaner`.
- One multi-thread tokio runtime, created on first use, runs every server and connection. Each native call enters it.
- `Provider.create` with a `MemoryBackend` passes its `Arc<MemoryBackend>` to the Rust Provider. With any other backend it passes a `JavaBackend`, which holds a global reference to the Java object.
- `JavaBackend` attaches the calling tokio thread to the JVM as a daemon thread and runs each call inside a JNI local frame, so references do not accumulate on long-lived threads.
- A dedicated thread, attached as a daemon, receives from the `PromptReceiver` and calls the Java dispatcher with a `Prompt` object. The object holds a reply handle: a `Box` with the `oneshot::Sender`. Answering takes the sender out and sends; freeing the handle without an answer drops the sender, which refuses the prompt. `isWithdrawn` is `Sender::is_closed`.
- `LocalServer.start` calls `local::bind` and spawns `local::serve`. `close` aborts that task and waits until it has ended, so the socket file is gone when `close` returns.
- Rust panics never cross the boundary: each native function catches them and throws `IllegalStateException`.
- Classes are looked up through the class loader of `Native`, remembered at `JNI_OnLoad`, so that threads started by Rust find the SDK classes under any class loader.
- `Native` first calls `System.loadLibrary("pwc_provider_jni")`; when the library is not on `java.library.path` it extracts `native/<os>-<arch>/<library>` from the jar to a temporary file.
- On Windows `LocalServer.start(provider[, id])` binds the pipe through `local::bind_pipe`; the directory overloads throw `UnsupportedOperationException`.
- A Java backend must not call the Provider: its methods run while the Provider holds its lock (section 5).

### 8.3 Value mapping

| Rust | Java |
|---|---|
| `Id`, `Text`, `Network`, `Scope`, `Address` | `String`, validated with the Rust `Deserialize` rules when entering Rust |
| `Hash256`, `bitcoin::Txid` | `String`, lower-case hex in display order |
| `bitcoin::OutPoint`, `Reserved` | `OutPoint(String txid, int vout)`, `Set<OutPoint>` |
| `bitcoin::Script`, `Hex` of a script | `byte[]`; `OutputSummary.scriptPubKey` stays a hex `String` as in Rust |
| `bitcoin::TxOut`, `(ScriptBuf, u64)` | `TxOut(long valueSats, byte[] scriptPubKey)` |
| `bitcoin::Transaction` | `byte[]` (consensus serialization) |
| `bitcoin::Psbt` | `byte[]` (BIP-174 serialization) |
| `Sats`, `SignedSats`, `u64` | `long` |
| `u32` | `int`; a negative value is refused when entering Rust |
| `Timestamp` | `Instant`, millisecond precision |
| `Option<T>` field | nullable field; `Option<T>` return value of a backend method: `Optional<T>` / `OptionalLong` |
| `&'static str` values | enums: `SyncState`, `AddressRole`, `AddressState`, `UtxoRestriction`, `TransactionStatus`; `AccountInfo.methods` are method names looked up in the Rust method registry, unknown names are dropped |
| `Vec<Value>` of `Provider::grants` | `List<Grant>` |
| `PwcError` | `PwcException` |
| `sign_psbt(&mut Psbt, ..) -> Vec<u32>` | `SignedPsbt signPsbt(String, byte[], List<Integer>)`; the returned PSBT replaces the input |

### 8.4 Dependencies

| Component | Use |
|---|---|
| `jni` 0.21.1 | JNI functions and calls into the JVM |
| `pwc-provider`, tokio, bitcoin, serde_json, time | As in section 7 |
| JUnit 5, Jackson (tests only) | Tests and the test LOCAL Client |

### 8.5 Build and distribution

- `./gradlew build` builds this platform's library with cargo. `-PnativeLibs=<dir>` instead takes prebuilt libraries laid out as `<os>-<arch>/<library>`.
- CI (`.github/workflows/ci.yml`):
  - The `java-sdk` matrix runs `ci/build.sh <platform>` on the four platforms. Each job builds, tests and uploads its library with `actions/upload-artifact`; run steps get no Buildkite agent token, so `buildkite-agent` cannot be called from them.
  - `java-sdk-jar` downloads the four libraries and runs `ci/assemble.sh`, which checks that the jar has `module-info.class` and all four libraries.
  - `java-sdk-publish` runs only on a tag `pwc-provider-java-v<version>`, after `java-sdk-jar`, and publishes the libraries downloaded from `java-sdk`, a job it therefore also lists in `needs`, with `ci/assemble.sh --publish <tag>`, using the `PWC_PACKAGES_TOKEN` secret. It is a job of its own because the runner resolves every secret a job references before the job starts.
- `provider-windows` runs the Rust tests on Windows, where they run over the named pipe.
- Jobs on one Buildkite agent share the home directory, so every Rust job outside Windows sets its own `CARGO_HOME` under the job's workspace before installing the toolchain; the toolchains in `~/.rustup` stay shared.

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
| pwc-provider (tests only) | The Provider for the end-to-end tests |

# Python binding of the Client (pwc-client-ffi)

## 1. Layout

```text
pwc-client/
├── uniffi.toml         how JSON values and paths appear in each language
└── src/ffi.rs          feature `uniffi`: scaffolding, custom types, adapters

pwc-client-ffi/
├── Cargo.toml          cdylib; depends on pwc-client with the feature `uniffi`
├── README.md           how to build and generate
├── src/lib.rs          re-exports the scaffolding of pwc-client
├── src/bin/uniffi-bindgen.rs
└── tests/
    ├── python.rs       builds the bindings, starts a Provider, runs the Python program
    └── test_bindings.py
```

## 2. Where the annotations are

The types and methods are annotated where they are defined, with `cfg_attr(feature = "uniffi", ...)`, so the binding cannot drift from the Rust interface and no type is declared twice. `pwc-client-ffi` only links the library into a C dynamic library and re-exports its scaffolding, which makes `pwc_client` the single generated module.

## 3. Adapters (`src/ffi.rs`)

The Rust interface was designed for this, so only what UniFFI cannot express directly is adapted:

| Rust interface | In the binding | Reason |
|---|---|---|
| `serde_json::Value` | Custom type over a string; `uniffi.toml` converts it with the language's JSON module | Not a UniFFI type |
| `PathBuf` in `Endpoint` | Custom type over a string | Not a UniFFI type |
| `discover(&Path)` | A function of the same name taking a string | Borrowed path |
| `Client::connect(&Endpoint, ...)`, `Client::pair(...)` | Constructors of the same names taking the record by value | Borrowed record |
| `Client::subscribe_events() -> EventStream` | Returns a shared handle | Objects cross the boundary as shared handles |
| `CredentialStore` with `&str` parameters | A trait of the same name with `String` parameters, wrapped to implement the Rust trait | Foreign implementations take owned values |
| `FileCredentialStore::new` | The function `file_credential_store`, returning a `CredentialStore` | So that it can be passed where a store is expected |
| `Connection` | Not exposed | Its methods consume the connection |

## 4. Runtime

Asynchronous methods are exported with UniFFI's Tokio support: the library runs them on its own runtime and completes the foreign future, so the Python event loop is never blocked. Calls into a Python `CredentialStore` are synchronous and short.

## 5. Dependencies

| Crate | Use |
|---|---|
| uniffi 0.32 (`tokio`) | Optional dependency of `pwc-client` |
| uniffi 0.32 (`cli`) | The binding generator program in `pwc-client-ffi` |
| pwc-provider, tempfile, tokio (tests only) | The Provider for the Python test |
