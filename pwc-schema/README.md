# PWC schemas and registry

Machine-readable companions of the PWC specifications. License: CC BY 4.0, except the `.py` scripts, which are MIT (PWC-GOVERNANCE-001 §2.2).

| File | Content |
|---|---|
| `pwc-core-0.2.1.schema.json` | Core types and every method's params and results; unknown members allowed |
| `pwc-core-0.2.1.strict.schema.json` | Same, rejecting unknown members; for testing Provider output |
| `pwc-transport-local-0.1.0.schema.json` | LOCAL frames and `local.*` methods |
| `pwc-transport-browser-0.1.0.schema.json` | BROWSER provider info, request arguments, rejection errors |
| `pwc-transport-remote-0.1.0.schema.json` | REMOTE Nostr events, plaintexts and `remote.*` methods |
| `pwc-registry-0.1.0.json` | Registry of all names (PWC-REGISTRY-001) |
| `gen_schema.py` | Regenerates the schema files |
| `gen_registry.py` | Regenerates the registry JSON and `specs/PWC-Registry-v0.1.0-EN.md` from the Core text |
| `check_schema.py` | Validates the schemas against every example in the specifications and against `../pwc-testkit/vectors/types.json` |

Each schema's `x-pwc-methods` maps a method name to `$defs` entries for its `params`, its direct `result` and, for methods that return an Operation, the `operationResult` of the completed Operation. Transport schemas reference the Core schema by `$id`; register all files with your validator.

```text
python3 gen_schema.py
python3 gen_registry.py ../specs/PWC-Core-v0.2.1-EN.md ../specs
python3 check_schema.py ../specs/PWC-Core-v0.2.1-EN.md ../specs/PWC-Transport-Local-v0.1.0-EN.md ../specs/PWC-Transport-Remote-v0.1.0-EN.md
```

The `$id` values use the placeholder domain `purity-wallet-connect.invalid` until a permanent location is assigned (PWC-GOVERNANCE-001, G-03).
