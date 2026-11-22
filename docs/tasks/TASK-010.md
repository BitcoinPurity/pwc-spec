# TASK-010: State save and load

Milestone: M9

## Goal

Offer `save_state` and `with_state` for the host wallet to call.

## Tests written first

- After saving and loading into a new Provider: old credentials authenticate
- Grants remain active
- Idempotent retries get the original responses
- Completed Operations remain readable
- Reservations remain in effect
- Operations awaiting approval become `failed` with `INTERNAL_ERROR`
- A corrupt snapshot or an unknown version is refused

## Implementation

- `Provider::save_state`
- `Provider::with_state`

## Acceptance criteria

- These tests pass

## Test identifiers covered

Core §8.5, §9.3; LOCAL §5.5
