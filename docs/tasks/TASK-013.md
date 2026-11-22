# TASK-013: In-memory backend and permission prompts

Milestone: M12

## Goal

Use `MemoryBackend` from Java and grant permissions.

## Tests written first

- `PermissionPrompt` carries the request; `grant` creates a grant visible in `Provider.grants()`; `reject` gives `rejected`
- `revokeGrant` makes later requests fail with `PERMISSION_REVOKED`
- The read methods answer with the in-memory wallets
- Each simulation control changes what the read methods report

## Implementation

- `MemoryBackend` with its controls and the `WalletBackend` methods
- Backend records and enums
- `PermissionPrompt`, `GrantChoice`, `Grant`

## Acceptance criteria

- These tests pass in `./gradlew build`
