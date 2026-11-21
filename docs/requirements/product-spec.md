# Product specification: pwc-provider

Kept consistent with `input/requirements.md`. The specifications in `specs/` are authoritative; this document records only this implementation's choices.

## 1. Deliverables

| Deliverable | Description |
|---|---|
| Library `pwc_provider` | For embedding in a host wallet: protocol layer, LOCAL transport, backend trait, in-memory backend, state save and load |
| Demo binary `pwc-provider` | Runs a LOCAL Provider on the in-memory backend, for Client development and conformance testing |

## 2. Features

### 2.1 LOCAL transport

- The endpoint directory is chosen as in LOCAL §3.2, or given with `--dir`. It must belong to the current user and grant no permission to group or others, or the Provider refuses to start. The socket file has mode 0600.
- If the socket path is longer than the operating system allows, the Provider refuses to start and does not fall back to another location.
- If a socket file exists, the Provider tries to connect to it. A refused connection means a stale file, which is replaced; an accepted one means another instance is running, and the Provider refuses to start.
- Every connection is checked for the peer's operating-system user and closed at once if it is another user.
- Frames: one JSON-RPC 2.0 object per line; `max_frame_bytes` is 1048576.
- Handshake: `local.hello`, then `local.pair` or `local.authenticate`, within 10 and 60 seconds.
- The pairing code has 6 digits. A pairing prompt unanswered for 300 seconds fails with `OPERATION_EXPIRED`; if the connection closes, the prompt is withdrawn and no credential is created.
- Failed authentications are rate-limited per peer process and answered after a delay.
- `local.unpair` deletes the credential, revokes all of the Client's grants and closes all of its connections.
- Event push: a Client holding `events:read` receives `pwc.event` notifications on each authenticated connection. A connection whose Client reads too slowly for its output buffer is closed.

### 2.2 Core methods

All 24 methods; see the method table of `docs/api-contract.md`. Requests are evaluated in the ten-step order of checks of Core §8.

### 2.3 In-memory backend

- One simulated chain per network, isolated from the others.
- Two wallets per network:
  - a software wallet with one P2WPKH and one P2PKH account, both able to sign;
  - a watch-only wallet with one P2WPKH account that cannot sign.
- Each account starts with confirmed UTXOs.
- The simulated chain supports broadcast (inputs must exist, be unspent and carry valid signatures), mining, reorganizations, replacement, an offline switch, and spends made by the User in the wallet itself.
- Wallets can be switched between `ready`, `locked` and `unavailable`.

### 2.4 User approval

The four kinds of User decision reach the host through the `Prompt` channel: pairing, permission requests, PSBT signing and message signing. A host that drops the reply refuses the request; the Provider withdraws a prompt by dropping its receiving end.

Demo binary:

- `--approve ask` (default): prints the prompt on the terminal and reads y/n.
- `--approve auto`: approves after `--approve-delay-ms` (default 300 ms) and prints a warning on standard error at startup. A permission request is granted as requested; without a requested network it is granted on `regtest`, and without requested wallets on every wallet of that network.

### 2.5 State save and load

- `Provider::save_state()` exports a JSON snapshot: paired Clients (including `client_secret`), grants, Operations, idempotency records, reservations and the cursor key.
- `Provider::with_state(...)` restores one, and fails on an unknown version or corrupt content.
- The snapshot contains secrets. The host chooses where to store it and protects it at least as well as its wallet files (LOCAL §7).
- On load, Operations still waiting for approval become `failed` with `INTERNAL_ERROR` and `retryable: true`; records past their retention are dropped.

## 3. Capabilities

| Field | Value |
|---|---|
| `profiles` | `pwc-core-read-v1`, `pwc-psbt-signer-v1` |
| `supported_networks` | `mainnet`, `signet`, `regtest` |
| `features` | `coin_control`, `watch_only`, `event_push` |
| `script_types` | `p2wpkh`, `p2pkh` |
| `message_formats` | `bip322-simple`, `legacy` |

The `limits` values are in `docs/api-contract.md`.

## 4. Continuous integration

`.github/workflows/ci.yml` has two jobs: specification checks (`check_schema.py`, `run_vectors.py`, `check_family.py`) and the Rust Provider (`cargo fmt --check`, `cargo clippy -D warnings` and `cargo test`, which includes running `local_runner.py` against the demo binary). The Rust toolchain is pinned to 1.97.0.

## 5. Known limitations

- Keeping state across restarts depends on the host calling the save and load functions. The demo binary must be paired again after a restart.
- `mainnet` and `signet` are in-memory simulations too.
- There is no official runner for the Core C-xx tests; this project's integration tests cover them, which is not a claim of official conformance.
- No Taproot, multisignature or delegation. `psbt.combine` returns `PSBT_CONFLICT` for conflicting Taproot signature fields without verifying them.
