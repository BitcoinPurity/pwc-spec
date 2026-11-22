# TASK-011: Crate scaffold, errors and wire types

## Goal

Create `pwc-client/` with the error types and the Core wire types, and a test helper that validates JSON against `pwc-schema/`.

## Tests first (`tests/types.rs`)

- Every `Sats`, `PositiveSats`, `SignedSats` and `FeeRate` vector of `pwc-testkit/vectors/types.json` parses exactly when it is valid.
- A `Balance` parses into integers and is written back as the same JSON.
- An error code outside Core §10 parses as `ErrorCode::Other`; a known code is written back unchanged.
- A result with an unknown member parses.
- Every fixture of `tests/common/fixtures.rs` satisfies the strict schema, parses, and is written back as the same JSON.
- The Operation accessors give the result, `OperationPending`, the Operation's error, or `ResultUnavailable`.

## Implementation

- `src/error.rs`: `ErrorCode`, `PwcError`, `Error`.
- `src/types.rs`: the types of Core §3 and §4, parameters and results of §8, and `HelloInfo` and `PairParams` of LOCAL §5.
- `tests/common/schema.rs`, `tests/common/fixtures.rs`.

## Done when

`cargo test --test types` passes.
