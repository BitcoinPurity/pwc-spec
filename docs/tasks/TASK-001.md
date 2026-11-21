# TASK-001: Project scaffold and schema validation helper

Milestone: M0

## Goal

Set up the Rust project and a test helper that validates any output against the strict schema.

## Tests written first

- Load the strict Core schema and the LOCAL schema from `pwc-schema/`, registered by `$id`, so that cross-file `$ref`s resolve
- Judge a known valid and a known invalid example correctly

## Implementation

- Root `.gitignore` (`target/`, `__pycache__/`)
- `pwc-provider/Cargo.toml` and an empty library
- `tests/common/schema.rs`: the schema validator

## Acceptance criteria

- `cargo test` passes

## Test identifiers covered

—
