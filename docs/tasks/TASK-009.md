# TASK-009: Event push

Milestone: M8

## Goal

Implement `pwc.event` delivery and filtering, and advertise `event_push`.

## Tests written first

- Delivery with and without `events:read`
- An unauthorized listener learns nothing protected
- No `account.*` or `wallet.syncChanged` events for a locked or unavailable wallet
- A Client that stops reading is disconnected when its buffer fills

## Implementation

- `Provider::notify`
- Event fan-out and filtering

## Acceptance criteria

- These tests pass

## Test identifiers covered

C-27, C-53, L-21, L-22
