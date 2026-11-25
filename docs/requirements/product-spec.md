# Product specification: pwc-provider and pwc-provider-java

Kept consistent with `input/requirements.md`. The specifications in `specs/` are authoritative; this document records only this implementation's choices.

## 1. Deliverables

| Deliverable | Description |
|---|---|
| Library `pwc_provider` | For embedding in a host wallet: protocol layer, LOCAL transport, backend trait, in-memory backend, state save and load |
| Demo binary `pwc-provider` | Runs a LOCAL Provider on the in-memory backend, for Client development and conformance testing |

## 2. Features

### 2.1 LOCAL transport

- The endpoint directory is chosen as in LOCAL §3.2, or given with `--dir`. It must belong to the current user and grant no permission to group or others, or the Provider refuses to start. The socket file has mode 0600.
- If the socket path is longer than the operating system allows, the Provider refuses to start and does not fall back to another location.
- If a socket file exists, the Provider tries to connect to it. A refused connection means a stale file, which is replaced; an accepted one means another instance is running, and the Provider refuses to start.
- On Windows the endpoint is the named pipe `\\.\pipe\pwc-<sid>-pwc-provider` (LOCAL §3.3), whose DACL grants the user's SID only, and `--dir` is not accepted. If the name exists, the Provider checks the SID of its server process: the same SID means another instance is running, any other SID that the name is taken. In both cases it refuses to start.
- Every connection is checked for the peer's operating-system user and closed at once if it is another user. The peer's process id and executable are recorded on Linux, macOS and Windows.
- Frames: one JSON-RPC 2.0 object per line; `max_frame_bytes` is 1048576.
- Handshake: `local.hello`, then `local.pair` or `local.authenticate`, within 10 and 60 seconds.
- The pairing code has 6 digits. A pairing prompt unanswered for 300 seconds fails with `OPERATION_EXPIRED`; if the connection closes, the prompt is withdrawn and no credential is created.
- Failed authentications are rate-limited per peer process and answered after a delay.
- `local.unpair` deletes the credential, revokes all of the Client's grants and closes all of its connections.
- Event push: a Client holding `events:read` receives `pwc.event` notifications on each authenticated connection. A connection whose Client reads too slowly for its output buffer is closed.

### 2.2 Core methods

All 24 methods; see the method table of `docs/api-contract.md`. Requests are evaluated in the ten-step order of checks of Core §8.

### 2.3 In-memory backend

- One simulated chain per network, isolated from the others.
- Two wallets per network:
  - a software wallet with one P2WPKH and one P2PKH account, both able to sign;
  - a watch-only wallet with one P2WPKH account that cannot sign.
- Each account starts with confirmed UTXOs.
- The simulated chain supports broadcast (inputs must exist, be unspent and carry valid signatures), mining, reorganizations, replacement, an offline switch, and spends made by the User in the wallet itself.
- Wallets can be switched between `ready`, `locked` and `unavailable`.

### 2.4 User approval

The four kinds of User decision reach the host through the `Prompt` channel: pairing, permission requests, PSBT signing and message signing. A host that drops the reply refuses the request; the Provider withdraws a prompt by dropping its receiving end.

Demo binary:

- `--approve ask` (default): prints the prompt on the terminal and reads y/n.
- `--approve auto`: approves after `--approve-delay-ms` (default 300 ms) and prints a warning on standard error at startup. A permission request is granted as requested; without a requested network it is granted on `regtest`, and without requested wallets on every wallet of that network.

### 2.5 State save and load

- `Provider::save_state()` exports a JSON snapshot: paired Clients (including `client_secret`), grants, Operations, idempotency records, reservations and the cursor key.
- `Provider::with_state(...)` restores one, and fails on an unknown version or corrupt content.
- The snapshot contains secrets. The host chooses where to store it and protects it at least as well as its wallet files (LOCAL §7).
- On load, Operations still waiting for approval become `failed` with `INTERNAL_ERROR` and `retryable: true`; records past their retention are dropped.

## 3. Capabilities

| Field | Value |
|---|---|
| `profiles` | `pwc-core-read-v1`, `pwc-psbt-signer-v1` |
| `supported_networks` | `mainnet`, `signet`, `regtest` |
| `features` | `coin_control`, `watch_only`, `event_push` |
| `script_types` | `p2wpkh`, `p2pkh` |
| `message_formats` | `bip322-simple`, `legacy` |

The `limits` values are in `docs/api-contract.md`.

## 4. Continuous integration

`.github/workflows/ci.yml` runs on every push to any branch and on manual dispatch; Buildkite executes it, and it receives only `push` events. It has three jobs: specification checks (`check_schema.py`, `run_vectors.py`, `check_family.py`); the Rust Provider (`cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`, which includes running `local_runner.py` against the demo binary); and coverage, which runs the same tests under `cargo-llvm-cov` and writes the per-file coverage table to the job summary (`$GITHUB_STEP_SUMMARY`) and the log. Coverage is reported only and never fails the build. The Rust toolchain is pinned to 1.97.0.

## 5. Known limitations

- Keeping state across restarts depends on the host calling the save and load functions. The demo binary must be paired again after a restart.
- `mainnet` and `signet` are in-memory simulations too.
- There is no official runner for the Core C-xx tests; this project's integration tests cover them, which is not a claim of official conformance.
- No Taproot, multisignature or delegation. `psbt.combine` returns `PSBT_CONFLICT` for conflicting Taproot signature fields without verifying them.

## 6. Java Provider SDK

Kept consistent with `input/requirements.md` §5 and §6. The Java SDK is a JNI binding of the library above: every protocol behavior, value and limit in sections 2 to 5 applies unchanged.

### 6.1 Deliverables

| Deliverable | Description |
|---|---|
| Java library `pwc-provider-java` | JPMS module and package `org.bitcoinpurity.pwc.provider`: Provider, LOCAL server, prompts, backend interface, in-memory backend. No dependencies. The published jar contains the native library `pwc_provider_jni` for linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64 and windows-x86_64; a local build contains that of the build platform |
| Maven artifact | `org.bitcoinpurity.pwc:pwc-provider-java` on GitHub Packages (`https://maven.pkg.github.com/BitcoinPurity/pwc-spec`), published by CI from a tag `pwc-provider-java-v<version>` |
| JNI crate `pwc-provider-jni` | `pwc-provider-java/native/`, a `cdylib` over `pwc-provider/` |
| Demo program | `org.bitcoinpurity.pwc.provider.demo.Main`, with the options of the Rust demo binary; `./gradlew run` or the `installDist` start script |

### 6.2 Features

- **Provider**: created from a `WalletBackend`, a `Config` and a `PromptHandler`, or restored from a snapshot. `saveState`, `pairedClients`, `grants`, `revokeGrant`, `unpairClient` and `notify` behave as their Rust counterparts.
- **Config**: `Config.defaults()` returns the Rust `Config::default()` values, read from the native library; a builder changes single fields.
- **LOCAL server**: `LocalServer.start` binds the endpoint as in section 2.1 and serves in the background; `close` stops accepting and removes the socket file. Connections already open keep running until the Client closes them, as in Rust.
- **Prompts**: each prompt reaches the `PromptHandler` on one dedicated thread, in order. The handler may answer at once or later, from any thread. `isWithdrawn()` tells whether the Provider withdrew it. A prompt that is never answered is refused when it is garbage-collected; a handler that throws refuses its prompt.
- **Java backend**: a host implements `WalletBackend` in Java with Java types, byte arrays and hex strings. An exception thrown by a backend method that can fail becomes the request's error: `PwcException` keeps its code, message, details and `retryable`; any other exception gives `INTERNAL_ERROR`. A method that cannot fail in the Rust trait (`wallets`, `account`, `addresses`, `ownsScript`, `isUnspent`, `feeEstimate`, `ownedOutput`) is treated as returning nothing when it throws, and the exception is printed.
- **In-memory backend**: `MemoryBackend.demo()` and all the simulation controls of section 2.3. It implements `WalletBackend`, so a host can wrap it; passed to a Provider directly, it is used without Java callbacks.
- **Resources**: `Provider`, `LocalServer` and `MemoryBackend` are `AutoCloseable`; native memory is also released when they are garbage-collected.

### 6.3 Requirements on the host

- Java 25 or later. Run with `--enable-native-access=ALL-UNNAMED` to avoid the JVM's native-access warning.
- The native library is loaded from `java.library.path` when it is there, for example installed next to a packaged application so that it can be signed; otherwise the jar's copy is extracted to a temporary file and loaded.
- As a module, enable native access with `--enable-native-access=org.bitcoinpurity.pwc.provider`, or the name of the module it is merged into.

### 6.4 Known limitations

- Platforms other than the five of the published jar are not supported.
- The library does not create PSBTs or signatures for a Java backend; a Java backend does that itself.

# PWC LOCAL Client (Rust)

## 1. Deliverable

| Deliverable | Description |
|---|---|
| Library `pwc_client` | For applications and for future language SDKs: LOCAL transport (Client side), typed Core methods, credential store interface, helpers |

## 2. Features

### 2.1 Discovery and connection

- `endpoint_directory()` gives the endpoint directory of LOCAL §3.2; `discover(dir)` lists the `*.sock` files in it as endpoints.
- Before connecting, the endpoint directory must belong to the current user and grant nothing to group or others; otherwise the library does not connect.
- `Connection::open` connects and sends `local.hello`. The Provider's name, version, `max_frame_bytes` and `delegation` are available from the connection.

### 2.2 Pairing and authentication

- `pairing_code()` returns 6 random decimal digits. The application shows the code, then calls `pair`, which returns when the User decides in the wallet.
- `authenticate` sends a fresh nonce and the client proof, and compares the server proof in constant time. On a mismatch it closes the connection without sending anything further.
- `Client::connect` and `Client::pair` do the same using a `CredentialStore`: `connect` fails with `NotPaired` when the store holds no credential for the Provider; `pair` saves the new credential; `unpair` deletes it.
- `FileCredentialStore` keeps one file per `provider_id`, created with mode 0600.
- The library never logs or prints a secret, proof or nonce.

### 2.3 Requests

- Any number of requests may be outstanding on a connection; responses are matched by `id`.
- A request larger than the Provider's `max_frame_bytes` is refused locally.
- One typed function per Core method (see `docs/api-contract.md`), and `request(method, params)` for anything else.
- A completed Operation's result is read with a typed accessor for its method. An Operation that ended in another terminal state gives its `PwcError`.

### 2.4 Helpers

- `idempotency_key()` returns a new random `Id`.
- `wait_operation` returns the Operation once it is terminal. It wakes on the Operation's `operation.stateChanged` event and otherwise polls once per second, until a caller-given time limit.
- `list_all_addresses`, `list_all_utxos` and `list_all_transactions` follow `next_cursor` to the last page.
- `subscribe_events` returns a stream that the application reads by pulling. A reader that falls more than 256 events behind is told that events were missed.

## 3. Tests and continuous integration

- Unit and integration tests run against a scripted Provider inside the tests, against the test kit's mock Provider, and end to end against the Rust Provider of `pwc-provider/` with its in-memory wallet.
- The `client` job of `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` for `pwc-client`.

## 4. Known limitations

- No automatic reconnection.
- The end-to-end tests use the Provider's in-memory wallet; no test runs against a wallet connected to a real node.
- macOS and Linux only.

# Python binding of the Client (pwc-client-ffi)

## 1. Deliverables

| Deliverable | Description |
|---|---|
| Feature `uniffi` of `pwc_client` | The UniFFI annotations of the public interface and the few adapters a binding needs |
| Library `pwc_client_ffi` | The C dynamic library that the generated bindings load |
| Program `uniffi-bindgen` | Generates the bindings from the built library |

## 2. Using it from Python

- Build and generate as `pwc-client-ffi/README.md` says. The result is `pwc_client.py` and the shared library in one directory; put that directory on the Python path and `import pwc_client`.
- Every method that is `async` in Rust is a coroutine; call it with `await` inside an `asyncio` event loop.
- Records are Python data classes with the Rust field names. Amounts are `int`. Enumerations are Python enumerations; an enumeration whose variants carry data, such as `FeePolicy` and `ErrorCode`, has one class per variant.
- A failure raises `pwc_client.Error`; each variant is a subclass, such as `Error.Pwc`, `Error.NotPaired` and `Error.ConnectionClosed`. `Error.Pwc` carries the `PwcError` as `e[0]`; its `code` is an `ErrorCode`.
- `CredentialStore` is a base class with `load`, `save` and `delete`. An exception raised by an implementation that is not a `pwc_client.Error` becomes `Error.Io` with the exception's text.
- The result of a terminal Operation is read with the methods of `Operation`, as in Rust.

## 3. Known limitations

- No wheel: the application ships the generated module and the shared library itself.
- macOS and Linux only.
