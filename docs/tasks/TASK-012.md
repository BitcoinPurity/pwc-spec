# TASK-012: Provider, LOCAL server and pairing prompts from Java

Milestone: M11

## Goal

Create a Provider, serve it over LOCAL and answer pairing prompts in Java.

## Tests written first

- `LocalServer.start` creates the directory with mode 0700 and the socket with mode 0600; `close` removes the socket
- Unsafe directory, overlong path and a running instance give the matching `EndpointException`; a stale socket is replaced
- An approved pairing prompt carries the claimed name, a 6-digit code and the peer process; the Client receives a credential and authenticates
- A rejected pairing gives `USER_REJECTED`
- A prompt is withdrawn when its connection closes
- A second answer throws `IllegalStateException`
- `pairedClients` and `unpairClient`

## Implementation

- `Provider`, `LocalServer`, `EndpointException`, `PromptHandler`, `Prompt`, `PairingPrompt`, request records
- Prompt thread and reply handles

## Acceptance criteria

- These tests pass in `./gradlew build`
