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
- Windows named pipes; the REMOTE, BROWSER and MOBILE transports.
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

Added after the Rust Provider. It builds on `pwc-provider/` and changes nothing in it.

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
- `pwc-provider/` is unchanged, and its CI job still passes.
- CI has a job for the Java SDK.
