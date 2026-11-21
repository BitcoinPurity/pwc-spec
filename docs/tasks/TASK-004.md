# TASK-004: Sessions, grants, Operations, idempotency and the order of checks

Milestone: M3

## Goal

Implement the method registry, the ten-step order of checks, `pwc.getCapabilities`, `session.*`, `operation.*`, `local.unpair` and the demo binary.

## Tests written first

- Enumerating wallets without a grant gives `PERMISSION_REQUIRED`
- An exact idempotent retry returns the same Operation; changed parameters give `IDEMPOTENCY_CONFLICT`
- Another Client's Operation gives `OPERATION_NOT_FOUND`
- A terminal Operation never changes
- A grant expires at `valid_until`
- A grant never exceeds its request
- An Operation still awaiting approval at its expiry becomes `expired`
- Exceeding the concurrency limit gives `RATE_LIMITED`
- Repeated requests after a refusal give `RATE_LIMITED`
- Revoking another Client's grant gives `GRANT_NOT_FOUND`
- Order of checks: a request failing several checks gets the error of the first
- Run `local_runner.py --pair --allow-destructive` against the demo binary with no failure

## Implementation

- `src/methods.rs`
- `src/provider/`
- `src/main.rs`

## Acceptance criteria

- These tests pass; the runner reports no failure

## Test identifiers covered

C-01, C-05, C-18, C-20, C-31, C-33, C-37, C-50, C-51, C-52, C-56, C-62, L-18, L-19, L-20, L-23
