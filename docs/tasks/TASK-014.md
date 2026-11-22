# TASK-014: Typed Core methods

## Goal

One typed function for each of the 24 methods of Core §8, and `request` for everything else.

## Tests first (`tests/core_methods.rs`)

- The fake Provider's answer for each of the 24 methods in the schema's `x-pwc-methods` satisfies the strict schema, including the result of a completed Operation.
- For every function: the method name and the exact `params` sent, which the fake Provider also checks against the schema, and the parsed fields of the result.
- Both forms of `FeePolicy`; optional parameters are omitted when absent; amounts are sent as decimal strings.
- The result accessors of a completed, a pending and a canceled Operation.

## Implementation

`src/client.rs`: `Client::request` and the typed functions.

## Done when

`cargo test --test core_methods` passes.
