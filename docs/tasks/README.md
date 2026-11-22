# Task index

Requirements: `input/requirements.md`. Each task writes its failing tests first, then the implementation.

# PWC LOCAL Client (Rust)

| Task | Title | Tests covered | Status |
|---|---|---|---|
| [TASK-011](TASK-011.md) | Crate scaffold, errors and wire types | C-22, C-26 (amount types) | Done |
| [TASK-012](TASK-012.md) | LOCAL endpoints and authentication proofs | L-12; LOCAL §3.2, §3.5 (Client side) | Done |
| [TASK-013](TASK-013.md) | Connection, handshake and events | L-13; LOCAL §4, §5.2 to §5.4, §8 (Client side) | Done |
| [TASK-014](TASK-014.md) | Typed Core methods | Core §8 (all 24 methods) | Done |
| [TASK-015](TASK-015.md) | Helpers and the mock Provider | LOCAL §5.6, §5.7, §7; Client Guide §2 to §4 | Done |

Test files are in `pwc-client/tests/`. Task numbers start at 011 because TASK-001 to TASK-010 belong to the Rust Provider.
