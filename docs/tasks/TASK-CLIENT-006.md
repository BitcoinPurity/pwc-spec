# TASK-CLIENT-006: End-to-end tests against the Rust Provider, and CI

## Goal

Run the library against the Rust Provider of `pwc-provider/`, and run the Client's checks in GitHub Actions.

## Tests first (`tests/provider.rs`)

The test starts a Provider on the in-memory demo wallet inside the test process, serves it on a socket in a temporary endpoint directory, and answers its approval prompts: it approves, or refuses when the test says so.

- Discover, pair and store the credential, then connect again with it.
- `get_capabilities` lists all 24 methods and `event_push`.
- A permission request for every scope completes after approval; the grant is on `regtest`.
- Wallets, accounts, balance, receive address, a created address, and all addresses, UTXOs and transactions of the P2WPKH account.
- A payment: `create_psbt`, `sign_psbt`, `finalize_psbt`, `broadcast_transaction`, then `get_transaction` and the changed balance. Waiting for the signature ends on the Operation's event, in less than the polling interval.
- A reservation is released.
- A message is signed and verifies; a changed message does not.
- A refused signature gives `USER_REJECTED` from the Operation's accessor.
- L-20: the connection is closed while `psbt.sign` waits for approval; after reconnecting, the same key returns the same Operation.
- The same key with other parameters gives `IDEMPOTENCY_CONFLICT`.
- A balance change in the wallet arrives as an `account.balanceChanged` event.
- After `revoke_permissions`, a read fails with `PERMISSION_REVOKED`.
- After `unpair`, the stored credential is gone and the old credential fails with `UNAUTHENTICATED`.

## Implementation

- `pwc-client/Cargo.toml`: `pwc-provider` as a development dependency.
- `.github/workflows/ci.yml`: the `client` job.

## Done when

`cargo test --test provider` passes, and the `client` job passes on the pull request.
