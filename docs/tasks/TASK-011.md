# TASK-011: Java SDK scaffold and native library loading

Milestone: M10

## Goal

Gradle 9.1.0 project, JNI crate, native library in the jar, `Config.defaults()` from Rust.

## Tests written first

- The native library loads from the jar
- `Config.defaults()` and `Limits.defaults()` equal the values of `docs/api-contract.md` §2 and LOCAL timeouts
- The builder changes one field and keeps the others

## Implementation

- `pwc-provider-java/` Gradle project and wrapper
- `native/` crate with `JNI_OnLoad` and the shared runtime
- `Native`, `NativeHandle`, `Config`, `Limits`, `RateLimit`

## Acceptance criteria

- These tests pass in `./gradlew build`
