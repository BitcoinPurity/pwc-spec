# TASK-014: Java wallet backends

Milestone: M13

## Goal

Drive the Provider with a `WalletBackend` written in Java.

## Tests written first

- With a Java backend that forwards to `MemoryBackend`, every Core read method answers exactly as with `MemoryBackend` itself, and every backend method is called
- `PwcException` thrown by the backend becomes the request's error; another exception gives `INTERNAL_ERROR`

## Implementation

- `JavaBackend` in the JNI crate
- `ErrorCode`, `PwcException`

## Acceptance criteria

- These tests pass in `./gradlew build`
