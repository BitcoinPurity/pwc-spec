# Requirements: PWC LOCAL Provider (Rust) and Java Provider SDK

This file is the single entry point for requirements. When a requirement changes, update this file first, then bring `docs/requirements/product-spec.md`, `docs/architecture.md`, `docs/api-contract.md` and `docs/tasks/` in line.

## 1. Goal

Implement the PWC server side, the Provider (wallet side), as specified in `specs/`:

- every one of the 24 methods of Core v0.2.1 (`specs/PWC-Core-v0.2.1-EN.md`);
- served over the LOCAL transport v0.1.0 (`specs/PWC-Transport-Local-v0.1.0-EN.md`): Unix socket, JSON-RPC 2.0, pairing, mutual HMAC-SHA256 authentication, event push.

The specifications define no HTTP interface. The LOCAL transport forbids any TCP or UDP listener.

## 2. Decisions

| Item | Decision |
|---|---|
| Language | Rust, a single crate in `pwc-provider/` |
| Methods | All Core methods, including `psbt.create`, `psbt.sign`, `psbt.combine`, `psbt.finalize`, `psbt.releaseReservation`, `transaction.broadcast`, `message.sign` and `message.verify` |
| Wallet backend | A backend trait; the first version ships an in-memory simulation and connects to no real node |
| Networks | `mainnet`, `signet`, `regtest`. Purity addresses use exactly the Bitcoin Core formats |
| Script types | `p2wpkh` (PSBT creation and signing), `p2pkh` (read methods and `legacy` message signing) |
| Message formats | `bip322-simple` (P2WPKH), `legacy` (P2PKH) |
| Persistence | The Provider writes no files. It offers library calls to save and load its state; the host wallet calls them and stores the snapshot |
| Event push | Implemented; `event_push` is advertised |
| User approval | Delivered to the host's UI through a channel. The demo binary can ask on the terminal or approve automatically |
| Method | Test first |

## 3. Out of scope

- Delegated principals (LOCAL §5.8): `local.openDelegate`, `local.delegated` and `local.closeDelegate` return `UNSUPPORTED_OPERATION`, and `local.hello` reports `delegation: false`.
- The REMOTE, BROWSER and MOBILE transports. (Windows named pipes were out of scope here and are added in section 6.)
- The `awaiting_unlock` state: Operation-returning methods on a locked wallet fail with `WALLET_LOCKED`.
- Request quotas (Core §8 check 2, a MAY).
- Taproot, multisignature accounts, hardware signers.
- Persistence in the demo binary.

## 4. Acceptance

- `cargo test` passes and covers the C-xx and L-xx test identifiers listed in `docs/tasks/README.md`.
- With the demo binary in automatic-approval mode, `pwc-testkit/local_runner.py --pair --allow-destructive` reports no failure.
- The process has no TCP or UDP listener.
- The repository's existing `run_vectors.py` and `check_schema.py` give the same results as before.
- The CI workflow (`.github/workflows/ci.yml`, run by Buildkite) runs on every push to any branch and on manual dispatch: the specification check scripts, and `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` for `pwc-provider`, and a line-coverage report of the test suite in the job summary, without a threshold.

## 5. Java Provider SDK

Added after the Rust Provider. It builds on `pwc-provider/` and changes nothing in its library. One of its tests is changed: L-22 (`tests/events.rs`) let its "fast" Client fall behind on a busy machine, so the test now lets that Client catch up after every 50 events.

### 5.1 Goal

Let a Java host wallet embed the Rust Provider through a JNI binding, with the whole host-facing interface of the Rust library available from Java:

- create a Provider with a `Config`, or restore one from a snapshot; save its state;
- serve it over the LOCAL transport, and stop serving;
- receive the four kinds of approval prompt and answer them;
- implement the wallet backend in Java, or use the in-memory backend and its simulation controls;
- list paired Clients and grants, revoke a grant, unpair a Client, report wallet changes as events;
- a demo program with the options of the Rust demo binary.

### 5.2 Decisions

| Item | Decision |
|---|---|
| Form | JNI binding of the Rust library: a Rust `cdylib` crate with the JNI functions, and a Java library over it |
| Location | `pwc-provider-java/`; the JNI crate is `pwc-provider-java/native/` and depends on `pwc-provider/` by path |
| Rust JNI crate | `jni` 0.21.1 |
| Java | Java 25 (LTS); Java package `org.bitcoinpurity.pwc.provider` |
| Build | Gradle 9.1.0 (wrapper, Kotlin DSL). The Gradle build runs `cargo build` and puts the native library in the jar |
| Bitcoin types in the Java interface | bitcoinj 0.17.1: `Sha256Hash` (txids), `TransactionOutPoint`, `Script`, `Transaction`, `TransactionOutput`. PSBTs are BIP-174 bytes |
| Platforms | Those of the Rust LOCAL transport: Linux and macOS. The jar carries the native library of the platform it was built on |
| Behavior | Exactly that of the Rust library: the Java layer adds no protocol logic |
| Method | Test first (JUnit 5) |

### 5.3 Out of scope

- Everything out of scope for the Rust Provider (section 3).
- Publishing the jar to a repository; jars with native libraries for several platforms.
- Rust modules that are not part of the host interface (`methods`, `psbt`, `message`, `balance`, `types`) and the `client_proof` and `server_proof` helpers.

### 5.4 Acceptance

- `./gradlew build` in `pwc-provider-java/` passes. Its tests cover every class of the Java interface, the Java backend callbacks for all backend methods, and the four prompt kinds.
- The Java demo program in automatic-approval mode passes `pwc-testkit/local_runner.py --pair --allow-destructive` with no failure.
- `cargo fmt --check` and `cargo clippy -D warnings` are clean for the JNI crate.
- The `pwc-provider` library is unchanged, and its CI jobs pass while the Java SDK job runs at the same time.
- CI has a job for the Java SDK.

## 6. Java Provider SDK 0.2: Windows, portable interface and distribution

Prepares the SDK for embedding in PurityWallet desktop (`PurityWallet/PurityWalletPC`). The wallet itself is not changed here. Where this section and section 5 differ, this section applies.

### 6.1 Goal

- The LOCAL transport also runs on Windows, over a named pipe (LOCAL §3.3, §3.4, test L-24), in the Rust library, its demo binary and the Java SDK.
- The Java interface uses only Java types, byte arrays and strings; the SDK has no third-party Java dependency.
- The SDK loads its native library from `java.library.path` first, and from the jar otherwise.
- The SDK is a JPMS module.
- One jar carries the native libraries of every supported platform.
- The jar is published to GitHub Packages.
- All code in this repository is licensed under MIT.

### 6.2 Decisions

| Item | Decision |
|---|---|
| Windows endpoint | `\\.\pipe\pwc-<sid>-<provider_id>`. The first instance is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, byte mode and a protected DACL granting access to the user's SID only |
| Existing pipe name | If the name exists, the Provider connects to it and checks the server process's SID: the same SID means another instance is running (`ALREADY_RUNNING`); any other SID, or no answer, means the name is taken and is not used (`NAME_TAKEN`) |
| Windows peer check | `GetNamedPipeClientProcessId` and the process token's user SID; executable from `QueryFullProcessImageNameW` |
| macOS peer executable | `proc_pidpath` |
| Java value types | Transaction ids: lower-case hex `String`s in display order. Outpoints: `OutPoint(String txid, int vout)`. Scripts, transactions and PSBTs: `byte[]`. Outputs: `TxOut(long valueSats, byte[] scriptPubKey)` |
| Java endpoint selection | `LocalServer.start(provider)` and `start(provider, providerId)` use the platform's default endpoint; `start(provider, dir, providerId)` exists on macOS and Linux only |
| Native library | Name `pwc_provider_jni`. Jar resources `org/bitcoinpurity/pwc/provider/native/<os>-<arch>/`, with `<os>` one of `linux`, `macos`, `windows` and `<arch>` one of `x86_64`, `aarch64` |
| Platforms in the published jar | linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64. Not macos-x86_64: there is no Intel Mac build agent |
| JPMS module | `org.bitcoinpurity.pwc.provider`, exporting `org.bitcoinpurity.pwc.provider` |
| Publishing | Maven coordinates `org.bitcoinpurity.pwc:pwc-provider-java:<version>` on `https://maven.pkg.github.com/BitcoinPurity/pwc-spec`. CI publishes on a tag `pwc-provider-java-v<version>`, with a token from a Buildkite secret. Version 0.2.0; 0.2.1 in section 7 |
| License | MIT for all code: `pwc-provider/`, `pwc-provider-java/`, `pwc-client/`, `pwc-client-ffi/`, the scripts in `pwc-schema/` and `pwc-testkit/`. Documents, schemas, the registry and test vectors stay CC BY 4.0 |
| CI | Rust tests on Linux and Windows. Java SDK build and tests on all four platforms, then one jar with all four native libraries |

### 6.3 Out of scope

- PurityWallet changes.
- Windows support in the Python test kit (`pwc_local_client.py`, `local_runner.py`), which opens `AF_UNIX` sockets only. Its run against the demo programs stays on macOS and Linux.
- Code signing of the native libraries.

### 6.4 Acceptance

- `cargo test` in `pwc-provider/` passes on Linux and on Windows; the Windows tests cover the pipe name, the DACL, `ALREADY_RUNNING`, the peer check and the protocol over the pipe.
- `./gradlew build` in `pwc-provider-java/` passes on all four platforms.
- The assembled jar contains the four native libraries, has a `module-info.class` and depends on no other artifact.
- `cargo clippy -D warnings` is clean for Linux and for the Windows target.

## 7. Java Provider SDK 0.2.1: the `provider_id` of `local.hello`

A fix. Where this section and sections 2 to 6 differ, this section applies.

### 7.1 Problem

`local.hello` always answered `provider_id` `pwc-provider`, whatever id the endpoint was bound with. A host that serves on its own id, for example `LocalServer.start(provider, "puritywallet")`, listens on `puritywallet.sock` but reports `pwc-provider`, and a Client that checks the id against the socket name (LOCAL §3.2, §5.2), such as `pwc-client`, refuses to connect.

### 7.2 Decisions

| Item | Decision |
|---|---|
| `provider_id` of `local.hello` | The id the endpoint was bound with: `local::bind(dir, provider_id)` or `local::bind_pipe(provider_id)` |
| Default id | `PROVIDER_ID` stays `pwc-provider`; the demo binary and `LocalServer.start(provider)` still use it |
| Java SDK version | 0.2.1, published on a tag `pwc-provider-java-v0.2.1`. The Java interface is unchanged |

### 7.3 Acceptance

- `cargo test` in `pwc-provider/` checks that `local.hello` reports the id of the endpoint, with an id other than `pwc-provider`.
- A Java test checks the same through `LocalServer.start(provider, providerId)`, on every platform.
- A `pwc-client` test connects to the Rust Provider bound with an id other than `pwc-provider`.
- The checks of section 6.4 still pass.

# PWC LOCAL Client (Rust)

## 1. Goal

Implement the PWC Client side as a library that applications import, as specified in `specs/`:

- the Client side of the LOCAL transport v0.1.0 (`specs/PWC-Transport-Local-v0.1.0-EN.md`): discovery, Unix socket, JSON-RPC 2.0, pairing, mutual HMAC-SHA256 authentication, event notifications;
- a typed function for each of the 24 methods of Core v0.2.1 (`specs/PWC-Core-v0.2.1-EN.md`);
- helpers for the tasks every Client has (`specs/PWC-Client-Guide-v0.1.0-EN.md`): storing the credential, waiting for an Operation, following pagination, reading events.

The library is the single implementation of the protocol logic. SDKs for other languages will later wrap it through a foreign-function interface; they are not part of this work.

## 2. Decisions

| Item | Decision |
|---|---|
| Language | Rust, a single crate in `pwc-client/` |
| Multi-language strategy | The public interface uses only shapes that a binding generator (UniFFI) maps directly: shared handles with `&self` async methods, plain data records, one error enumeration, events read by pulling |
| Transport | LOCAL only, on macOS and Linux |
| Methods | A typed function for every Core method, and a raw `request(method, params)` |
| Credential storage | A `CredentialStore` interface implemented by the host; the crate ships a file store (mode 0600) |
| Idempotency keys | Supplied by the application for every intended action; the library offers a generator and never chooses a key by itself |
| Dependencies | The library does not depend on `pwc-provider`; the tests use it as a development dependency |
| End-to-end tests | Against the Rust Provider of `pwc-provider/` with its in-memory wallet, started inside the test process, with the test answering the Provider's approval prompts |
| Continuous integration | A `client` job in `.github/workflows/ci.yml`: `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` for `pwc-client` |
| Method | Test first |

## 3. Out of scope

- Language bindings and their packaging.
- Automatic reconnection: after `ConnectionClosed` the application connects again and retries with the same idempotency key.
- Windows named pipes; the REMOTE, BROWSER and MOBILE transports.
- Typed functions for delegated principals (LOCAL §5.8); `request` can send them.
- Operating-system credential stores (Keychain, Secret Service): the host implements `CredentialStore`.

## 4. Acceptance

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass in `pwc-client/`.
- Every request the library sends validates against `pwc-schema/`; every result fixture used by the tests validates against the strict schema before the library parses it.
- The library pairs, authenticates and calls Core methods against `pwc-testkit/mock_local_provider.py`.
- Against the Rust Provider, the library completes the flow of the Client guide: pair, reconnect, obtain a grant, read the account, create, sign, finalize and broadcast a transaction, sign and verify a message, receive events, and handle a refusal, a revocation and unpairing.
- The `client` job of GitHub Actions passes.
- Test L-13 holds: after a wrong `server_proof` the Client sends nothing further and closes.
- The repository's existing `run_vectors.py`, `check_schema.py` and `check_family.py` give the same results as before.

# Python binding of the Client (pwc-client-ffi)

## 1. Goal

Let Python applications import the Client. The binding is generated from `pwc-client` with UniFFI, so the protocol logic stays in the Rust library. Python is the first language; the same annotations serve the other languages UniFFI generates later.

## 2. Decisions

| Item | Decision |
|---|---|
| Generator | UniFFI 0.32, procedural macros, library mode |
| Annotations | In `pwc-client`, behind the optional feature `uniffi`; without the feature the library is unchanged and has no UniFFI dependency |
| Shared library | The crate `pwc-client-ffi/` builds the C dynamic library and holds the binding generator program |
| Python module | `pwc_client`, one generated file next to the shared library |
| Interface | The same names as the Rust interface: `Client` with the 24 Core methods, `request`, the helpers, `EventStream`, the data types and `Error` |
| Asynchronous methods | Python coroutines, run on the library's own Tokio runtime |
| JSON values | `request` parameters and results, `Operation.result`, `PwcEvent.payload` and `PwcError.details` are Python objects (`dict`, `list`, ...), converted with the `json` module |
| Credential storage | The application implements `CredentialStore` in Python, for example over the `keyring` package; the file store is also available |
| Low-level handshake | `Connection` is not exposed; Python uses `Client.connect` and `Client.pair` |
| Method | Test first |

## 3. Out of scope

- Bindings for other languages.
- Packaging as a wheel and publishing to a package index.
- Windows.

## 4. Acceptance

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass in `pwc-client-ffi/`, and `pwc-client` passes the same with and without the feature `uniffi`.
- `cargo test` in `pwc-client-ffi/` generates the Python module and runs a Python program against the Rust Provider of `pwc-provider/`: pair with a credential store written in Python, reconnect, obtain a grant, read the account, create, sign, finalize and broadcast a transaction, receive an event, and handle errors by their code.
- GitHub Actions runs these checks.
