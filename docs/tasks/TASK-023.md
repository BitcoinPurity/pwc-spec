# TASK-023: Backend calls without the state lock

Milestone: M21

## Goal

A backend call that waits no longer holds up the handshake, pairing, prompt answers, the host's calls or `notify`, and `notify` may be called from inside a backend method. Backend methods still never run concurrently.

## Tests written first

- `pwc-provider/tests/backend_lock.rs`, with a backend over the in-memory one that can hold a call, call `notify` from inside a call, count concurrent calls and panic:
  - while a call is held: another Client authenticates, `paired_clients`, `grants`, `revoke_grant`, `save_state` and `notify` return
  - `notify` from inside a backend call does not deadlock and its event arrives
  - parallel requests of several Clients and `notify` calls never run two backend calls at once
  - a backend panic gives `INTERNAL_ERROR` and the next request is answered
- `pwc-provider-java`: `JavaBackendTest` calls `Provider.notify` from inside a Java backend method

## Implementation

- `parking_lot` state lock released around every backend call with `MutexGuard::unlocked`; a backend lock taken before it
- Core requests and approval decisions with `spawn_blocking`
- An event thread for `notify` and the re-evaluation after a revocation or expiry
- Javadoc of `WalletBackend` and `Provider.notify`

## Acceptance criteria

- `input/requirements.md` §8.3
