# TASK-007: psbt.create, reservations and broadcast

Milestone: M6

## Goal

Implement coin selection and PSBT construction, input reservations and broadcast on the simulated chain.

## Tests written first

- One input and two outputs give vsize 141, and a fee of 177 at 1250 sat/kvB
- An address of another network gives `NETWORK_MISMATCH`
- The PSBT carries no data beyond its transaction
- Reserved UTXOs are reported as `locked` and become selectable after expiry
- Releasing twice gives `true`, then `false`
- Releasing another Client's reservation gives `RESERVATION_NOT_FOUND`
- A User spend of a reserved output ends the reservation
- Broadcast accepted, refused and repeated

Note: An accepted and a repeated broadcast need a signed transaction, so they are checked in the end-to-end test of TASK-008 (`tests/core_sign.rs`).

## Implementation

- Coin selection and the simulated chain in `src/memory.rs`
- The reservation table
- The three method handlers

## Acceptance criteria

- These tests pass

## Test identifiers covered

C-15, C-19, C-39, C-44, C-46 to C-49, C-54, C-64
