# TASK-020: Native loading, JPMS module and Windows in the Java SDK

Milestone: M19

## Goal

Load the native library from `java.library.path` first, ship a module, run on Windows.

## Tests written first

- `Native` loads from `java.library.path` when the library is there
- The jar has `module-info.class` for `org.bitcoinpurity.pwc.provider`
- `LocalServer.start(provider)` uses the platform's default endpoint
- The Java tests pass on Windows over the named pipe

## Implementation

- `Native`, `module-info.java`, `LocalServer`, test Client over named pipes

## Acceptance criteria

- `input/requirements.md` §6.4
