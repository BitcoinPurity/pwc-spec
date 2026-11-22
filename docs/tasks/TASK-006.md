# TASK-006: psbt.combine and psbt.finalize

Milestone: M5

## Goal

Implement the pure PSBT combination and finalization.

## Tests written first

- PSBT-01 to PSBT-08 of `vectors/psbt.json`, comparing PSBTs as key-value sets
- An out-of-range `vout` does not crash
- `max_psbt_bytes` and `max_combine_psbts` exceeded

## Implementation

- Raw parsing, combine and finalize in `src/psbt.rs`
- Both method handlers

## Acceptance criteria

- These tests pass

## Test identifiers covered

C-11 to C-14, C-30, C-40, C-55
