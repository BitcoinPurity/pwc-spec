# TASK-FFI-002: pwc-client-ffi, the Python module and its test

## Goal

Build the C dynamic library, generate the Python module `pwc_client`, and prove it against the Rust Provider.

## Tests first

`tests/python.rs` generates the module into a temporary directory, starts a Provider on the in-memory wallet whose prompts are approved after 200 ms, and runs `tests/test_bindings.py`, which must exit with success. The Python program checks:

- `discover` finds the Provider; `Client.connect` raises `Error.NotPaired` with an empty store.
- `Client.pair` saves the credential in a store written in Python; `Client.connect` then authenticates with it.
- `get_capabilities`; a permission request on which `wait_operation` first raises `Error.Timeout` after 50 ms and then completes; `permissions_result`.
- Wallets, accounts and a balance whose amounts are `int`.
- A payment: `create_psbt` with `FeePolicy.RATE`, `sign_psbt`, `finalize_psbt`, `broadcast_transaction`, `get_transaction`.
- An `operation.stateChanged` event from `subscribe_events`, with a `dict` payload.
- `request` with a `dict`, returning a `dict`.
- `Error.Pwc` with `ErrorCode.WALLET_NOT_FOUND`; an Operation accessor on a pending Operation raises `Error.OperationPending`.
- An exception raised by the Python store becomes `Error.Io`.
- The store from `file_credential_store` saves and loads, with file mode 0600, and `Client.connect` accepts it.
- `unpair` empties the store, and the old credential then fails with `UNAUTHENTICATED`.

## Implementation

- `pwc-client-ffi/`: `Cargo.toml`, `src/lib.rs`, `src/bin/uniffi-bindgen.rs`, `README.md`.
- `.github/workflows/ci.yml`: the feature in the `client` job, and the `client-ffi` job.

## Done when

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass in `pwc-client-ffi/`, and the `client-ffi` job passes.
