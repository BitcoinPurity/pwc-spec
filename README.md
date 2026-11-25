# Purity Wallet Connect (PWC)

Purity Wallet Connect is a family of specifications that lets applications use a Bitcoin Purity wallet without ever holding its keys. An application asks; the wallet shows the User exactly what would happen, computed from the transaction itself; the User decides.

**Status:** review drafts. Nothing here is a deployed standard yet, and incompatible changes are still possible.

## Repository layout

| Folder | Contents |
|---|---|
| [`specs/`](specs/) | The specification documents |
| [`pwc-schema/`](pwc-schema/) | JSON Schemas (2020-12), the machine-readable registry and their generators |
| [`pwc-testkit/`](pwc-testkit/) | Test vectors, PSBT fixtures, the LOCAL conformance runner, a mock Provider and a minimal Client |
| [`pwc-provider/`](pwc-provider/) | A Rust Provider implementing Core and LOCAL, with an in-memory demo wallet (requirements in [`input/`](input/), design in [`docs/`](docs/)) |
| [`pwc-provider-java/`](pwc-provider-java/) | The Java Provider SDK: a JNI binding of `pwc-provider/` for Java host wallets, with a demo program |
| [`pwc-client/`](pwc-client/) | A Rust Client library: Core v0.2.1 over the LOCAL transport; see [`docs/api-contract.md`](docs/api-contract.md) |
| [`pwc-client-ffi/`](pwc-client-ffi/) | Bindings of `pwc-client/` for other languages, generated with UniFFI; Python first |

## Documents

Start with the [Overview](specs/PWC-Overview-v0.1.0-EN.md), which explains how the documents depend on each other and which ones each kind of product implements.

| Document | Version | Purpose |
|---|---|---|
| [PWC-CORE-001](specs/PWC-Core-v0.2.1-EN.md) | 0.2.1 | Data types, methods, permissions, Operations, errors and events, independent of any platform |
| [PWC-TRANSPORT-LOCAL-001](specs/PWC-Transport-Local-v0.1.0-EN.md) | 0.1.0 | Native applications on the same computer |
| [PWC-TRANSPORT-BROWSER-001](specs/PWC-Transport-Browser-v0.1.0-EN.md) | 0.1.0 | Web pages through a browser extension |
| [PWC-TRANSPORT-REMOTE-001](specs/PWC-Transport-Remote-v0.1.0-EN.md) | 0.1.0 | Different devices over Nostr relays |
| [PWC-TRANSPORT-MOBILE-001](specs/PWC-Transport-Mobile-v0.1.0-EN.md) | 0.1.0 | Applications on the same phone |
| [PWC-CONFORMANCE-001](specs/PWC-Conformance-v0.1.0-EN.md) | 0.1.0 | Conformance classes, test kit and report format |
| [PWC-REGISTRY-001](specs/PWC-Registry-v0.1.0-EN.md) | 0.1.0 | Every name and identifier the family defines |
| [PWC-GOVERNANCE-001](specs/PWC-Governance-v0.1.0-EN.md) | 0.1.0 | License, roles, change process, security reports |
| [PWC-SECURITY-001](specs/PWC-Security-v0.1.0-EN.md) | 0.1.0 | Threat model and audit checklist (informative) |
| [PWC-UX-001](specs/PWC-UX-Guidelines-v0.1.0-EN.md) | 0.1.0 | Approval interface guidelines (informative) |
| [PWC-CLIENT-GUIDE-001](specs/PWC-Client-Guide-v0.1.0-EN.md) | 0.1.0 | Building a Client (informative) |
| [PWC-OVERVIEW-001](specs/PWC-Overview-v0.1.0-EN.md) | 0.1.0 | Map of the family (informative) |

## Running the checks

Python 3.10 or later. The schema check needs the `jsonschema` and `referencing` packages.

```text
python3 pwc-schema/check_schema.py specs/PWC-Core-v0.2.1-EN.md specs/PWC-Transport-Local-v0.1.0-EN.md specs/PWC-Transport-Remote-v0.1.0-EN.md
python3 pwc-testkit/run_vectors.py
python3 pwc-testkit/check_family.py specs
```

The Rust Provider needs a Rust toolchain; its tests include a run of the LOCAL runner against the demo binary:

```text
cd pwc-provider && cargo test
cargo run -- --dir /tmp/pwc/run --approve auto
```

The Java Provider SDK needs Java 25 and the same Rust toolchain; Gradle builds the native library and runs the tests, including the LOCAL runner against the Java demo program on macOS and Linux. It runs on Linux, macOS and Windows (over a named pipe), and is published as `org.bitcoinpurity.pwc:pwc-provider-java` on GitHub Packages with the native libraries of five platforms:

```text
cd pwc-provider-java && ./gradlew build
./gradlew run --args="--dir /tmp/pwc/run --approve auto"
```

The LOCAL runner, the mock Provider and the PSBT fixture checker are described in [`pwc-testkit/README.md`](pwc-testkit/README.md) and in PWC-CONFORMANCE-001 §4.

## Contributing and security

Changes are proposed and reviewed as described in PWC-GOVERNANCE-001 §4. Report suspected vulnerabilities in a specification privately, not in public issues (PWC-GOVERNANCE-001 §7).

## License

The documents and the data files (schemas, registry, test vectors) are licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). All code, that is the scripts in `pwc-schema/` and `pwc-testkit/`, `pwc-provider/`, `pwc-provider-java/`, `pwc-client/` and `pwc-client-ffi/`, is licensed under the [MIT License](LICENSE) (PWC-GOVERNANCE-001 §2.2).
