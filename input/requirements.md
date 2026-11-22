# Requirements: PWC LOCAL Provider (Rust)

This file is the single entry point for requirements. When a requirement changes, update this file first, then bring `docs/requirements/product-spec.md`, `docs/architecture.md`, `docs/api-contract.md` and `docs/tasks/` in line.

## 1. Goal

Implement the PWC server side, the Provider (wallet side), as specified in `specs/`:

- every one of the 24 methods of Core v0.2.1 (`specs/PWC-Core-v0.2.1-EN.md`);
- served over the LOCAL transport v0.1.0 (`specs/PWC-Transport-Local-v0.1.0-EN.md`): Unix socket, JSON-RPC 2.0, pairing, mutual HMAC-SHA256 authentication, event push.

The specifications define no HTTP interface. The LOCAL transport forbids any TCP or UDP listener.

## 2. Decisions

| Item | Decision |
|---|---|
| Language | Rust, a single crate in `pwc-provider/` |
| Methods | All Core methods, including `psbt.create`, `psbt.sign`, `psbt.combine`, `psbt.finalize`, `psbt.releaseReservation`, `transaction.broadcast`, `message.sign` and `message.verify` |
| Wallet backend | A backend trait; the first version ships an in-memory simulation and connects to no real node |
| Networks | `mainnet`, `signet`, `regtest`. Purity addresses use exactly the Bitcoin Core formats |
| Script types | `p2wpkh` (PSBT creation and signing), `p2pkh` (read methods and `legacy` message signing) |
| Message formats | `bip322-simple` (P2WPKH), `legacy` (P2PKH) |
| Persistence | The Provider writes no files. It offers library calls to save and load its state; the host wallet calls them and stores the snapshot |
| Event push | Implemented; `event_push` is advertised |
| User approval | Delivered to the host's UI through a channel. The demo binary can ask on the terminal or approve automatically |
| Method | Test first |

## 3. Out of scope

- Delegated principals (LOCAL §5.8): `local.openDelegate`, `local.delegated` and `local.closeDelegate` return `UNSUPPORTED_OPERATION`, and `local.hello` reports `delegation: false`.
- Windows named pipes; the REMOTE, BROWSER and MOBILE transports.
- The `awaiting_unlock` state: Operation-returning methods on a locked wallet fail with `WALLET_LOCKED`.
- Request quotas (Core §8 check 2, a MAY).
- Taproot, multisignature accounts, hardware signers.
- Persistence in the demo binary.

## 4. Acceptance

- `cargo test` passes and covers the C-xx and L-xx test identifiers listed in `docs/tasks/README.md`.
- With the demo binary in automatic-approval mode, `pwc-testkit/local_runner.py --pair --allow-destructive` reports no failure.
- The process has no TCP or UDP listener.
- The repository's existing `run_vectors.py` and `check_schema.py` give the same results as before.
- The CI workflow (`.github/workflows/ci.yml`, run by Buildkite) runs on every push to any branch and on manual dispatch: the specification check scripts, and `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` for `pwc-provider`.
