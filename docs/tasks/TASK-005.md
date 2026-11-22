# TASK-005: Backend trait and read methods

Milestone: M4

## Goal

Define `WalletBackend`, implement the data and read paths of the in-memory backend, and every read method with pagination.

## Tests written first

- BAL-01 to BAL-05 of `vectors/balance.json`
- Only approved account data is returned
- Locked and unavailable wallets
- An account outside the grant and a nonexistent account give identical responses
- A cursor reused with another account gives `INVALID_ARGUMENT`
- A new account of a granted wallet is covered
- Confirmations are recomputed after a reorganization
- A replaced transaction is not counted twice
- An unknown txid gives `TRANSACTION_NOT_FOUND`
- Wallets of the three networks are isolated by the grant's network
- `address.create` is idempotent

## Implementation

- `src/backend.rs`
- `src/balance.rs`
- The read paths of `src/memory.rs`
- The read method handlers

## Acceptance criteria

- These tests pass

## Test identifiers covered

C-02, C-16, C-17, C-24, C-25, C-34, C-35, C-36, C-38, C-57, C-58
