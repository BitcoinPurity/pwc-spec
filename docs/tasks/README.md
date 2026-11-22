# Task index

Requirements: `input/requirements.md`. Each task writes its failing tests first, then the implementation.

# PWC LOCAL Client (Rust)

| Task | Title | Tests covered | Status |
|---|---|---|---|
| [TASK-CLIENT-001](TASK-CLIENT-001.md) | Crate scaffold, errors and wire types | C-22, C-26 (amount types) | Done |
| [TASK-CLIENT-002](TASK-CLIENT-002.md) | LOCAL endpoints and authentication proofs | L-12; LOCAL §3.2, §3.5 (Client side) | Done |
| [TASK-CLIENT-003](TASK-CLIENT-003.md) | Connection, handshake and events | L-13; LOCAL §4, §5.2 to §5.4, §8 (Client side) | Done |
| [TASK-CLIENT-004](TASK-CLIENT-004.md) | Typed Core methods | Core §8 (all 24 methods) | Done |
| [TASK-CLIENT-005](TASK-CLIENT-005.md) | Helpers and the mock Provider | LOCAL §5.6, §5.7, §7; Client Guide §2 to §4 | Done |

Test files are in `pwc-client/tests/`. Client tasks are numbered `TASK-CLIENT-0xx`.
