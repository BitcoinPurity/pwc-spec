# TASK-016: State, demo program and CI

Milestone: M15

## Goal

Persistence from Java, the Java demo program and the CI job.

## Tests written first

- After `saveState` and `withState`, the old credential authenticates and the grant remains
- A corrupt snapshot and an unknown version give the matching `StateException`
- `local_runner.py --pair --allow-destructive` against the Java demo program: 16 pass, the rest manual

## Implementation

- `Provider.withState`, `saveState`, `StateException`
- `demo.Main`
- `.github/workflows/ci.yml` job for the Java SDK

## Acceptance criteria

- These tests pass in `./gradlew build`
