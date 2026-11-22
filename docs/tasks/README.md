# Task index

Requirements: `input/requirements.md`. Each task writes its failing tests first, then the implementation.

| Task | Title | Milestone | Tests covered | Status |
|---|---|---|---|---|
| [TASK-001](TASK-001.md) | Project scaffold and schema validation helper | M0 | — | Done |
| [TASK-002](TASK-002.md) | Error codes, semantic types, fee formula and authentication proofs | M1 | C-22, C-23, C-26, C-29, L-12 | Done |
| [TASK-003](TASK-003.md) | LOCAL transport and handshake | M2 | L-01, L-03 to L-11, L-14 to L-17, L-25 to L-28 | Done |
| [TASK-004](TASK-004.md) | Sessions, grants, Operations, idempotency and the order of checks | M3 | C-01, C-05, C-18, C-20, C-31, C-33, C-37, C-50, C-51, C-52, C-56, C-62, L-18, L-19, L-20, L-23 | Done |
| [TASK-005](TASK-005.md) | Backend trait and read methods | M4 | C-02, C-16, C-17, C-24, C-25, C-34, C-35, C-36, C-38, C-57, C-58 | Done |
| [TASK-006](TASK-006.md) | psbt.combine and psbt.finalize | M5 | C-11 to C-14, C-30, C-40, C-55 | Done |
| [TASK-007](TASK-007.md) | psbt.create, reservations and broadcast | M6 | C-15, C-19, C-39, C-44, C-46 to C-49, C-54, C-64 | Done |
| [TASK-008](TASK-008.md) | psbt.sign and message signing | M7 | C-03, C-04, C-06 to C-10, C-32, C-41 to C-43, C-45, C-59 to C-61, C-63 | Done |
| [TASK-009](TASK-009.md) | Event push | M8 | C-27, C-53, L-21, L-22 | Done |
| [TASK-010](TASK-010.md) | State save and load | M9 | Core §8.5, §9.3; LOCAL §5.5 | Done |

Test files are in `pwc-provider/tests/`. C-15 and C-54 (an accepted and a repeated broadcast) need a signed transaction and are checked by the end-to-end test in `core_sign.rs`.

Not applicable: C-21 (no external signer), L-13 (a Client-side test), L-24 (Windows), L-29 to L-34 (delegation; L-29's `UNSUPPORTED_OPERATION` is checked in `core_access.rs`). L-02 needs a second operating-system user and remains a manual test.

## Java Provider SDK

Requirements: `input/requirements.md` §5. Tests are JUnit 5 tests in `pwc-provider-java/src/test/java/`; `ForwardingBackend` is a Java backend over the in-memory one, and `DemoRunnerTest` runs `pwc-testkit/local_runner.py` against the Java demo program.

| Task | Title | Milestone | Status |
|---|---|---|---|
| [TASK-011](TASK-011.md) | Java SDK scaffold and native library loading | M10 | Done |
| [TASK-012](TASK-012.md) | Provider, LOCAL server and pairing prompts from Java | M11 | Done |
| [TASK-013](TASK-013.md) | In-memory backend and permission prompts | M12 | Done |
| [TASK-014](TASK-014.md) | Java wallet backends | M13 | Done |
| [TASK-015](TASK-015.md) | Signing prompts and events | M14 | Done |
| [TASK-016](TASK-016.md) | State, demo program and CI | M15 | Done |

# PWC LOCAL Client (Rust)

| Task | Title | Tests covered | Status |
|---|---|---|---|
| [TASK-CLIENT-001](TASK-CLIENT-001.md) | Crate scaffold, errors and wire types | C-22, C-26 (amount types) | Done |
| [TASK-CLIENT-002](TASK-CLIENT-002.md) | LOCAL endpoints and authentication proofs | L-12; LOCAL §3.2, §3.5 (Client side) | Done |
| [TASK-CLIENT-003](TASK-CLIENT-003.md) | Connection, handshake and events | L-13; LOCAL §4, §5.2 to §5.4, §8 (Client side) | Done |
| [TASK-CLIENT-004](TASK-CLIENT-004.md) | Typed Core methods | Core §8 (all 24 methods) | Done |
| [TASK-CLIENT-005](TASK-CLIENT-005.md) | Helpers and the mock Provider | LOCAL §5.6, §5.7, §7; Client Guide §2 to §4 | Done |
| [TASK-CLIENT-006](TASK-CLIENT-006.md) | End-to-end tests against the Rust Provider, and CI | L-20; Client Guide §2, §3, §7, §8 | Done |

Test files are in `pwc-client/tests/`. Client tasks are numbered `TASK-CLIENT-0xx`.
