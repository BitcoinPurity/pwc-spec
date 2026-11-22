# Product specification

Kept consistent with `input/requirements.md`. The specifications in `specs/` are authoritative; this document records only this implementation's choices.

# PWC LOCAL Client (Rust)

## 1. Deliverable

| Deliverable | Description |
|---|---|
| Library `pwc_client` | For applications and for future language SDKs: LOCAL transport (Client side), typed Core methods, credential store interface, helpers |

## 2. Features

### 2.1 Discovery and connection

- `endpoint_directory()` gives the endpoint directory of LOCAL §3.2; `discover(dir)` lists the `*.sock` files in it as endpoints.
- Before connecting, the endpoint directory must belong to the current user and grant nothing to group or others; otherwise the library does not connect.
- `Connection::open` connects and sends `local.hello`. The Provider's name, version, `max_frame_bytes` and `delegation` are available from the connection.

### 2.2 Pairing and authentication

- `pairing_code()` returns 6 random decimal digits. The application shows the code, then calls `pair`, which returns when the User decides in the wallet.
- `authenticate` sends a fresh nonce and the client proof, and compares the server proof in constant time. On a mismatch it closes the connection without sending anything further.
- `Client::connect` and `Client::pair` do the same using a `CredentialStore`: `connect` fails with `NotPaired` when the store holds no credential for the Provider; `pair` saves the new credential; `unpair` deletes it.
- `FileCredentialStore` keeps one file per `provider_id`, created with mode 0600.
- The library never logs or prints a secret, proof or nonce.

### 2.3 Requests

- Any number of requests may be outstanding on a connection; responses are matched by `id`.
- A request larger than the Provider's `max_frame_bytes` is refused locally.
- One typed function per Core method (see `docs/api-contract.md`), and `request(method, params)` for anything else.
- A completed Operation's result is read with a typed accessor for its method. An Operation that ended in another terminal state gives its `PwcError`.

### 2.4 Helpers

- `idempotency_key()` returns a new random `Id`.
- `wait_operation` returns the Operation once it is terminal. It wakes on the Operation's `operation.stateChanged` event and otherwise polls once per second, until a caller-given time limit.
- `list_all_addresses`, `list_all_utxos` and `list_all_transactions` follow `next_cursor` to the last page.
- `subscribe_events` returns a stream that the application reads by pulling. A reader that falls more than 256 events behind is told that events were missed.

## 3. Known limitations

- No automatic reconnection.
- The test kit's mock Provider never approves a permission request, so signing and broadcast flows are tested only against fixtures validated by the strict schema.
- macOS and Linux only.
