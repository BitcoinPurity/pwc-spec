# TASK-015: Helpers and the mock Provider

## Goal

The credential store with `Client::connect`, `Client::pair` and `Client::unpair`; `idempotency_key`; `wait_operation`; `list_all_*`. Run the library against `pwc-testkit/mock_local_provider.py`.

## Tests first

`tests/helpers.rs`, against the fake Provider:

- `idempotency_key` gives distinct values that satisfy `Id`.
- `FileCredentialStore`: file mode 0600, directory mode 0700, `save` replaces, `delete` of a missing credential succeeds, a `provider_id` with a path separator is refused.
- `connect` without a credential gives `NotPaired` and sends nothing; `pair` saves; `connect` then authenticates; `unpair` deletes.
- `wait_operation`: three polls one second apart when no event arrives; an immediate poll on the Operation's own event and none on another Operation's; `Timeout`; an error of `operation.get` is returned.
- `list_all_*` follow three pages and send the cursors received; `CURSOR_EXPIRED` is returned.

`tests/mock_provider.rs`, against the mock Provider started with `python3`:

- discover, pair and store, reconnect with the stored credential, `get_capabilities`, `get_session_info`, `request_permissions` (awaiting approval), the same key again (same `operation_id`), the same key with other parameters (`IDEMPOTENCY_CONFLICT`), `list_wallets` (`PERMISSION_REQUIRED`), overlapping requests, `unpair`, then `UNAUTHENTICATED`;
- with `--reject-pairing`: `USER_REJECTED` and nothing stored.

## Implementation

`src/credential.rs`: `CredentialStore`, `FileCredentialStore`. `src/client.rs`: the helpers.

## Done when

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass in `pwc-client/`, and the repository's `check_schema.py`, `run_vectors.py` and `check_family.py` give the same results as before.
