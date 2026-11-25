# PWC test kit

Described in PWC-CONFORMANCE-001 §3 to §5. Python 3.10 or later. Vector files are CC BY 4.0; the scripts are MIT (PWC-GOVERNANCE-001 §2.2).

| File | Purpose |
|---|---|
| `vectors/types.json` | Valid and invalid values per semantic type |
| `vectors/misc.json` | Fees, LOCAL authentication, REMOTE SAS, REMOTE splitting |
| `vectors/balance.json` | Balance scenarios with expected snapshots |
| `vectors/psbt.json` | PSBT fixtures and expected combine and finalize outcomes |
| `run_vectors.py`, `balance_reference.py` | Prove the vectors with independent reference code |
| `gen_psbt_fixtures.py`, `check_psbt_fixtures.py` | Build and independently verify the PSBT fixtures (need embit) |
| `local_runner.py` | Automated LOCAL tests against a running Provider; writes a report |
| `mock_local_provider.py` | Mock LOCAL Provider (Linux, macOS) |
| `pwc_local_client.py` | Minimal LOCAL Client used by the runner and the examples |
| `example_local_client.py` | The example of PWC-CLIENT-GUIDE-001 §4 |
| `check_family.py` | Editorial consistency check across all PWC documents |

```text
python3 run_vectors.py
python3 check_psbt_fixtures.py <path-to-embit/src>
python3 mock_local_provider.py --dir /tmp/pwc-test &
python3 local_runner.py --socket /tmp/pwc-test/mockwallet.sock --pair --allow-destructive --report report.json
python3 check_family.py ../specs
```

Against the mock Provider the runner passes all 16 automatable tests and lists 18 as `manual`.
