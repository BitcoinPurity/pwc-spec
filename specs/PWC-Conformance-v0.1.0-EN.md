# Purity Wallet Connect (PWC)
## Conformance Classes, Test Kit and Reporting

**Specification:** PWC-CONFORMANCE-001  
**Version:** 0.1.0 (review draft)  
**Date:** 2026-11-21  
**Language:** English (normative draft)  
**License:** CC BY 4.0  
**Covers:** PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001, -BROWSER-001, -REMOTE-001 and -MOBILE-001 v0.1.0  
**Status:** Proposed specification

> **Scope rule:** This document defines what an implementation claims when it says it conforms to PWC, which tests back each claim, the test kit that runs or supports them, and the report that records the outcome. The tests themselves are defined in the specifications they belong to; this document only groups them, adds executable vectors and fixes the reporting format.

## 0. Decisions

- Conformance is claimed per **class** (§2). An implementation may claim several.
- Every test keeps the identifier of the specification that defines it: `C-` for Core, `L-` LOCAL, `B-` BROWSER, `R-` REMOTE, `M-` MOBILE.
- Tests on methods or features an implementation does not advertise are reported as `na` (Core §13).
- Claims are self-assessed and published with a report in the format of §6. There is no certification body in this version (PWC-GOVERNANCE-001 §8).

Requirement keywords have the meanings given in PWC-CORE-001 §0.4.

## 1. Requirements for a claim

A conformance claim MUST name the class or classes, the exact versions of the specifications, and the implementation's name and version. It MUST be accompanied by a report (§6) in which every test of the claimed classes has an outcome. A claim is valid only if no test has the outcome `fail`.

## 2. Conformance classes

| Class | Implementation | Specifications | Required tests |
|---|---|---|---|
| **P-CORE** | Any Provider | Core, profile `pwc-core-read-v1` | C-01 to C-64, each as applicable to the advertised methods and features |
| **P-SIGNER** | Provider with signing accounts | Core, profile `pwc-psbt-signer-v1` | P-CORE, with every signing test applicable |
| **P-LOCAL** | LOCAL Provider | LOCAL | P-CORE; L-01 to L-12, L-14 to L-28 |
| **P-LOCAL-DELEGATION** | LOCAL Provider with delegation | LOCAL §5.8 | P-LOCAL; L-29 to L-34 |
| **C-LOCAL** | LOCAL Client | LOCAL | L-12, L-13; the directory check of LOCAL §3.2; the server check of LOCAL §3.3 on Windows |
| **X-BROWSER** | Browser extension in extension mode | BROWSER | P-CORE; B-01 to B-13, B-17, B-18 |
| **X-BRIDGE** | Browser extension and bridge host in bridge mode | BROWSER §7, LOCAL | B-01 to B-18; the bridge host also meets C-LOCAL |
| **P-REMOTE** | REMOTE Provider | REMOTE | P-CORE; R-03, R-05 to R-12, R-14, R-15, R-18 |
| **C-REMOTE** | REMOTE Client | REMOTE | R-01 to R-13, R-15 to R-17, each as it applies to the Client side |
| **P-MOBILE** | MOBILE Provider | MOBILE, REMOTE | P-REMOTE; M-03, M-04, M-07 to M-10 |
| **C-MOBILE** | MOBILE Client | MOBILE, REMOTE | C-REMOTE; M-01 to M-03, M-05, M-06 |

## 3. Test kit

The kit is distributed with the specifications. Paths are relative to the root of the distribution, where the documents themselves are in `specs/`. Its files:

| File | Purpose |
|---|---|
| `pwc-schema/pwc-core-0.2.1.schema.json` | Core types and method parameters and results; unknown members allowed |
| `pwc-schema/pwc-core-0.2.1.strict.schema.json` | The same, rejecting unknown members; for testing Provider output (C-28) |
| `pwc-schema/pwc-transport-local-0.1.0.schema.json` | LOCAL frames and `local.*` methods |
| `pwc-schema/pwc-transport-browser-0.1.0.schema.json` | BROWSER provider info, request arguments, rejection errors |
| `pwc-schema/pwc-transport-remote-0.1.0.schema.json` | REMOTE events, plaintexts and `remote.*` methods |
| `pwc-schema/pwc-registry-0.1.0.json` | Machine-readable registry (PWC-REGISTRY-001) |
| `pwc-schema/gen_schema.py`, `gen_registry.py` | Generators of the files above |
| `pwc-schema/check_schema.py` | Validates the schemas against every example in the specifications and the type vectors |
| `pwc-testkit/vectors/types.json` | Valid and invalid values per semantic type |
| `pwc-testkit/vectors/misc.json` | Fee, LOCAL authentication, REMOTE SAS and splitting vectors |
| `pwc-testkit/vectors/balance.json` | Balance scenarios and expected snapshots |
| `pwc-testkit/vectors/psbt.json` | PSBT fixtures and expected `psbt.combine` and `psbt.finalize` outcomes |
| `pwc-testkit/run_vectors.py`, `balance_reference.py` | Independent reference computations proving the vectors |
| `pwc-testkit/gen_psbt_fixtures.py`, `check_psbt_fixtures.py` | Generator and independent checker of the PSBT fixtures (need the embit library) |
| `pwc-testkit/local_runner.py` | Automated LOCAL tests against a running Provider (§5.2) |
| `pwc-testkit/mock_local_provider.py`, `pwc_local_client.py` | Mock Provider and minimal Client, for testing Clients and the runner itself |
| `pwc-testkit/example_local_client.py` | The example of PWC-CLIENT-GUIDE-001 §4 |

## 4. Running the kit

```text
python3 pwc-schema/check_schema.py specs/PWC-Core-v0.2.1-EN.md specs/PWC-Transport-Local-v0.1.0-EN.md specs/PWC-Transport-Remote-v0.1.0-EN.md
python3 pwc-testkit/run_vectors.py
python3 pwc-testkit/check_psbt_fixtures.py <path-to-embit/src>
python3 pwc-testkit/local_runner.py --socket <endpoint> --pair [--allow-destructive] --report report.json
```

The scripts need Python 3.10 or later; the schema check also needs the `jsonschema` and `referencing` packages. Against the mock Provider, `local_runner.py` passes all 16 automatable tests and lists the remaining 18 as `manual`.

## 5. Test procedures

### 5.1 Vectors

An implementation under test feeds each vector to its own code and compares the outcome:

| Vector set | What the implementation does | Tests |
|---|---|---|
| `types.json` | Accept exactly the values marked valid for each type | C-22, C-26, C-29 |
| `misc.json` fees | Compute the nominal minimum fee | C-23 |
| `misc.json` LOCAL authentication | Compute both proofs | L-12 |
| `misc.json` SAS | Compute the SAS | R-03 |
| `misc.json` splitting | Split a message of the given size | R-09 |
| `balance.json` | Report the expected `Balance` fields for the scenario | C-24, C-25 |
| `psbt.json` | Run each case's method on the named PSBTs; compare PSBTs as sets of key-value pairs | C-11 to C-14, C-40, C-55 |

### 5.2 LOCAL runner

`local_runner.py` connects to a Provider's endpoint, runs every LOCAL test that needs neither a person nor a second operating-system user, and writes a report. With `--pair` it first pairs a test Client, which makes the Provider show a pairing prompt for a person to approve. With `--allow-destructive` it also runs L-23, which deletes that pairing.

### 5.3 Manual tests

Tests the runner marks `manual`, and all BROWSER, REMOTE and MOBILE tests in this version, are run by a tester following the test's row in its specification. The tester records the outcome in the report, with a note describing the setup: for example, the second user account used for L-02, or the relay used for R-08.

### 5.4 Validating Provider output

Every response a Provider returns during testing SHOULD be validated against the strict Core schema. Any unknown member found this way fails C-28.

## 6. Report format

A report is a JSON object:

```json
{
  "suite": "PWC-TRANSPORT-LOCAL-001 v0.1.0 §11",
  "runner": "local_runner.py 0.1.0",
  "implementation": {"name": "PurityWallet", "version": "1.2.0"},
  "classes": ["P-CORE", "P-LOCAL"],
  "target": "/run/user/1000/pwc/puritywallet.sock",
  "date": "2026-11-21T00:00:00Z",
  "results": [
    {"id": "L-03", "outcome": "pass", "notes": ""},
    {"id": "L-02", "outcome": "manual", "notes": "needs a second operating-system user"}
  ]
}
```

| Member | Meaning |
|---|---|
| `suite` | The test set, by specification and section |
| `runner` | Tool and version, or `"manual"` |
| `implementation`, `classes` | The claim (§1). Added by the claimant if the runner did not write them |
| `results[].outcome` | One of `pass`, `fail`, `skip` (not run), `manual` (to be run by a person), `na` (not applicable because not advertised) |

A report supporting a claim MUST contain no `skip` or `manual` outcomes: every such test is run and its outcome replaced before the report is published.

## 7. Limitations of this version

| ID | Limitation |
|---|---|
| CA-01 | No automated runners for BROWSER, REMOTE or MOBILE; their tests are manual |
| CA-02 | PSBT fixtures cover one 2-of-2 P2WSH input only; no Taproot, legacy or multi-input fixtures |
| CA-03 | No automated Core behavior runner; the Core tests are run through a transport by hand or with the implementation's own harness |
| CA-04 | The LOCAL runner and mock Provider run on Linux and macOS only |

## 8. References

- PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001, -BROWSER-001, -REMOTE-001 and -MOBILE-001 v0.1.0
- PWC-REGISTRY-001, PWC-GOVERNANCE-001 v0.1.0
- JSON Schema 2020-12: https://json-schema.org/draft/2020-12
- embit: https://github.com/diybitcoinhardware/embit

---

**End of PWC-CONFORMANCE-001 v0.1.0 English review draft.**
