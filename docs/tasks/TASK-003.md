# TASK-003: LOCAL transport and handshake

Milestone: M2

## Goal

Implement the endpoint, the peer check, framing, the JSON-RPC envelope, `local.hello`, `local.pair` and `local.authenticate`.

## Tests written first

- Refusal to listen on an unsafe endpoint directory; overlong path; stale socket versus running instance
- First request other than hello; unknown protocol; hello and authentication time limits
- Pairing approved, refused, unanswered, and withdrawn without a credential when the connection closes; the prompt marks claims as unverified and shows peer information
- Wrong proof and unknown client_id give identical responses and close the connection; a replayed proof fails
- Overlong frame, invalid JSON, batch, duplicate outstanding id

## Implementation

- `src/local.rs`
- The pairing part of `src/prompt.rs`
- The Provider's table of paired Clients

## Acceptance criteria

- These tests pass

## Test identifiers covered

L-01, L-03 to L-11, L-14 to L-17, L-25 to L-28
