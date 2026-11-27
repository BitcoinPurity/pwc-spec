# TASK-022: `local.hello` reports the endpoint's `provider_id`

Milestone: M21

## Goal

`local.hello` answers the `provider_id` the endpoint was bound with, so that a Client accepts a Provider served on its own id. Java SDK version 0.2.1.

## Tests written first

- `pwc-provider/tests/local_transport.rs`: the test Harness binds an id other than `pwc-provider`, and `local.hello` reports that id
- `pwc-provider-java`: `LocalServerTest.defaultEndpointOfThePlatform` checks the `provider_id` of `local.hello` for `LocalServer.start(provider, providerId)`
- `pwc-client/tests/provider.rs`: the end-to-end tests run against the Rust Provider bound with an id other than `pwc-provider`

## Implementation

- `local::Endpoint` keeps its `provider_id`; `local::serve` passes it to each connection, which answers it in `local.hello`
- Java SDK version 0.2.1

## Acceptance criteria

- `input/requirements.md` §7.3
