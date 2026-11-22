# TASK-FFI-001: UniFFI annotations and adapters in pwc-client

## Goal

Make the public interface of `pwc-client` exportable by UniFFI behind the feature `uniffi`, without changing the library when the feature is off.

## Tests first

- The existing tests of `pwc-client` pass with and without the feature.
- The Python test of TASK-FFI-002 fails until the interface is exported.

## Implementation

- `Cargo.toml`: the optional dependency and the feature.
- `src/types.rs`, `src/error.rs`, `src/credential.rs`, `src/local.rs`, `src/client.rs`: `cfg_attr` annotations on the records, enumerations, the error, `Client`, `EventStream` and the methods of `Operation`.
- `src/ffi.rs`: the scaffolding, the custom types and the adapters listed in `docs/architecture.md`.
- `uniffi.toml`: JSON values as Python objects.

## Done when

`cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features` pass in `pwc-client/`.
