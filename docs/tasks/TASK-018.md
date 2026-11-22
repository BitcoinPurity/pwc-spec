# TASK-018: Windows named pipe endpoint

Milestone: M17

## Goal

Serve LOCAL over a named pipe on Windows (LOCAL §3.3, §3.4, L-24).

## Tests written first

- The pipe is `\\.\pipe\pwc-<sid>-<provider_id>` and its DACL grants the user's SID only
- A second Provider with the same id gets `ALREADY_RUNNING`
- The protocol tests run over the pipe
- A Client of the same user is accepted, with its process id and executable recorded
- macOS records the peer's executable

## Implementation

- `local::bind_pipe`, peer check, accept loop
- Unix-only parts behind `cfg(unix)`; demo binary on Windows
- Test harness Client over Unix sockets and named pipes

## Acceptance criteria

- `input/requirements.md` §6.4
