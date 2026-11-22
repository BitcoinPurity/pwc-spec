# TASK-015: Signing prompts and events

Milestone: M14

## Goal

PSBT and message signing approved in Java, and events reported by the host.

## Tests written first

- `PsbtSignPrompt` carries outputs, fee and the PSBT; after `approve`, finalize and broadcast succeed through a Java backend
- A rejected signature gives `rejected`
- `MessageSignPrompt` carries the exact message; the signature verifies with `message.verify`
- `notify` delivers `pwc.event` to a Client holding `events:read`

## Implementation

- `PsbtSignPrompt`, `MessageSignPrompt`, `BackendEvent`

## Acceptance criteria

- These tests pass in `./gradlew build`
