# Requirements

This file is the single entry point for requirements. When a requirement changes, update this file first, then bring `docs/requirements/product-spec.md`, `docs/architecture.md`, `docs/api-contract.md` and `docs/tasks/` in line.

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
| Dependencies | None on `pwc-provider`; developed on `main` independently of the Provider |
| Method | Test first |

## 3. Out of scope

- Language bindings and their packaging.
- Automatic reconnection: after `ConnectionClosed` the application connects again and retries with the same idempotency key.
- Windows named pipes; the REMOTE, BROWSER and MOBILE transports.
- Typed functions for delegated principals (LOCAL §5.8); `request` can send them.
- Operating-system credential stores (Keychain, Secret Service): the host implements `CredentialStore`.
- A continuous-integration workflow, and end-to-end tests against the Rust Provider, until that Provider is on `main`.

## 4. Acceptance

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass in `pwc-client/`.
- Every request the library sends validates against `pwc-schema/`; every result fixture used by the tests validates against the strict schema before the library parses it.
- The library pairs, authenticates and calls Core methods against `pwc-testkit/mock_local_provider.py`.
- Test L-13 holds: after a wrong `server_proof` the Client sends nothing further and closes.
- The repository's existing `run_vectors.py`, `check_schema.py` and `check_family.py` give the same results as before.
