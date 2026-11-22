# TASK-CLIENT-003: Connection, handshake and events

## Goal

A connection that multiplexes requests, maps errors, performs `local.hello`, `local.pair` and `local.authenticate`, and delivers `pwc.event` notifications.

## Tests first (`tests/local_transport.rs`, against `tests/common/fake_provider.rs`)

The fake Provider performs the real handshake and checks every frame it receives against the LOCAL schema and every `params` against the method's schema.

- `local.hello` then `local.pair` gives a Client and the credential; the parameters sent are those of LOCAL §12.1.
- `local.authenticate` succeeds with the stored credential and fails with `UNAUTHENTICATED` for a wrong secret.
- L-13: after a wrong `server_proof` the result is `ProviderAuthFailed`, the Provider receives no further frame and sees the connection closed.
- A Provider whose `provider_id` differs from its socket name is refused.
- Code `1000` gives `Pwc`, `-32600` gives `Rpc`, a result of the wrong shape gives `Protocol`, an unknown code is kept, `params` that is not an object is refused.
- Eight overlapping requests answered in reverse order each get their own answer.
- Events that arrive while a request is outstanding are delivered in order.
- A stream more than 256 events behind gets `EventsMissed`, then continues.
- When the Provider closes the connection, outstanding and later requests and event streams get `ConnectionClosed`.
- `close` ends the connection.
- A request longer than `max_frame_bytes` gives `FrameTooLarge` and is not sent.
- `Credential`'s `Debug` output does not contain the secret; `Client` and `EventStream` are `Send + Sync`.

## Implementation

`src/local.rs`: the reader task, `Link`, `Connection`. `src/credential.rs`: `Credential`.

## Done when

`cargo test --test local_transport` passes.
