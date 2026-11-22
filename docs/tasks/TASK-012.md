# TASK-012: LOCAL endpoints and authentication proofs

## Goal

Find Providers as LOCAL §3 requires of a Client, and compute the proofs of LOCAL §5.4.

## Tests first

- `tests/local_transport.rs`: the `local_auth` vectors of `pwc-testkit/vectors/misc.json` (L-12); `discover` lists only `*.sock` files with a valid `provider_id`, sorted, gives an empty list for a missing directory and refuses a directory with mode 0755; `Connection::open` refuses such a directory before sending anything.
- `src/local.rs` unit tests: the endpoint directory for `XDG_RUNTIME_DIR` owned by the user, unset, relative and missing, and for an unset `HOME`; the pairing code has 6 digits.

## Implementation

`src/local.rs`: `endpoint_directory`, `discover`, the directory check, `client_proof`, `server_proof`, `pairing_code`.

## Done when

The tests above pass.
