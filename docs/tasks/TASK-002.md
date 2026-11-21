# TASK-002: Error codes, semantic types, fee formula and authentication proofs

Milestone: M1

## Goal

Implement the semantic types of Core §3.2, the error codes of §10, the fee formula of §3.4 and the HMAC proofs of LOCAL §5.4.

## Tests written first

- Every primitive-type vector in `vectors/types.json`
- FEE-01 to FEE-07 in `vectors/misc.json`
- Both proofs of LAUTH-01

## Implementation

- `src/error.rs`
- `src/types.rs`
- `local::client_proof` and `local::server_proof`

## Acceptance criteria

- All these vectors pass; the list of skipped composite-type vectors is exactly the expected one

## Test identifiers covered

C-22, C-23, C-26, C-29, L-12
