"""Generate the PWC registry: pwc-registry-0.1.0.json and PWC-Registry-v0.1.0-EN.md.

Usage: python3 gen_registry.py <PWC-Core.md> <output-dir-for-markdown>
Error meanings, scope descriptions and event payloads are read from the Core text so that the registry cannot
drift from it. The method list is checked against the generated Core schema.
"""
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
core_md = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
out_dir = pathlib.Path(sys.argv[2])


def section(start, end):
    return core_md[core_md.index(start):core_md.index(end)]


def core_relative(cell):
    """Qualify a bare section reference copied from Core, e.g. "(§4.7)" -> "(Core §4.7)"."""
    return re.sub(r"(?<!Core )(?<!LOCAL )(?<!BROWSER )(?<!REMOTE )(?<!MOBILE )(?<!-001 )§", "Core §", cell)


def rows(text, first_col_pattern):
    out = []
    for line in text.splitlines():
        m = re.match(rf"^\| `({first_col_pattern})` \| (.*) \|$", line)
        if m:
            out.append([m.group(1)] + [core_relative(c.strip()) for c in m.group(2).split(" | ")])
    return out


errors = rows(section("## 10. Standard errors", "## 11."), r"[A-Z_]+")
scopes = rows(section("## 6. Permission scopes", "## 7."), r"[a-z]+:[a-z]+")
events = rows(section("### 8.6", "## 9."), r"[a-z]+\.[a-zA-Z]+")

CORE = "PWC-CORE-001 v0.2.1"
LOCAL = "PWC-TRANSPORT-LOCAL-001 v0.1.0"
REMOTE = "PWC-TRANSPORT-REMOTE-001 v0.1.0"
BROWSER = "PWC-TRANSPORT-BROWSER-001 v0.1.0"
MOBILE = "PWC-TRANSPORT-MOBILE-001 v0.1.0"

READ_V1 = {"pwc.getCapabilities", "session.getInfo", "session.requestPermissions", "session.getPermissions",
           "session.revokePermissions", "wallet.list", "account.list", "account.getBalance",
           "address.getReceiveAddress", "transaction.list", "operation.get", "operation.cancel"}

# name, scope, returns Operation, idempotency key, defined in
methods = [
    ("pwc.getCapabilities", None, False, False, CORE + " §8.1"),
    ("session.getInfo", None, False, False, CORE + " §8.1"),
    ("session.requestPermissions", None, True, True, CORE + " §8.1"),
    ("session.getPermissions", None, False, False, CORE + " §8.1"),
    ("session.revokePermissions", None, False, True, CORE + " §8.1"),
    ("wallet.list", "wallets:read", False, False, CORE + " §8.1"),
    ("account.list", "wallets:read", False, False, CORE + " §8.1"),
    ("account.getBalance", "balances:read", False, False, CORE + " §8.2"),
    ("address.getReceiveAddress", "addresses:read", False, False, CORE + " §8.2"),
    ("address.create", "addresses:create", False, True, CORE + " §8.2"),
    ("address.list", "addresses:read", False, False, CORE + " §8.2"),
    ("utxo.list", "utxos:read", False, False, CORE + " §8.2"),
    ("transaction.list", "transactions:read", False, False, CORE + " §8.2"),
    ("transaction.get", "transactions:read", False, False, CORE + " §8.2"),
    ("psbt.create", "psbt:create", True, True, CORE + " §8.3"),
    ("psbt.releaseReservation", "psbt:create", False, True, CORE + " §8.3"),
    ("psbt.sign", "psbt:sign", True, True, CORE + " §8.3"),
    ("psbt.combine", None, False, False, CORE + " §8.3"),
    ("psbt.finalize", None, False, False, CORE + " §8.3"),
    ("transaction.broadcast", "transactions:broadcast", True, True, CORE + " §8.3"),
    ("message.sign", "messages:sign", True, True, CORE + " §8.4"),
    ("message.verify", None, False, False, CORE + " §8.4"),
    ("operation.get", None, False, False, CORE + " §8.5"),
    ("operation.cancel", None, False, True, CORE + " §8.5"),
    ("local.hello", None, False, False, LOCAL + " §5.2"),
    ("local.pair", None, False, False, LOCAL + " §5.3"),
    ("local.authenticate", None, False, False, LOCAL + " §5.4"),
    ("local.unpair", None, False, False, LOCAL + " §5.7"),
    ("local.openDelegate", None, False, False, LOCAL + " §5.8"),
    ("local.delegated", None, False, False, LOCAL + " §5.8"),
    ("local.closeDelegate", None, False, False, LOCAL + " §5.8"),
    ("remote.ping", None, False, False, REMOTE + " §6"),
    ("remote.updateRelays", None, False, False, REMOTE + " §6"),
    ("remote.unpair", None, False, False, REMOTE + " §6"),
]
notifications = [
    ("pwc.event", "Provider to Client", LOCAL + " §8; " + REMOTE + " §5.3"),
    ("local.delegatedEvent", "Provider to delegator", LOCAL + " §5.8"),
    ("remote.paired", "Provider to Client", REMOTE + " §4.3"),
    ("remote.pairingRejected", "Provider to Client", REMOTE + " §4.2"),
    ("remote.relaysChanged", "Provider to Client", REMOTE + " §5.2"),
]
features = [
    ("coin_control", "`psbt.create` honors `selected_utxos`"),
    ("multisig", "The Provider handles multisignature accounts"),
    ("hardware_signing", "The Provider can sign through hardware devices"),
    ("watch_only", "The Provider handles watch-only accounts"),
    ("event_push", "The Provider delivers the events of Core §8.6 through a transport profile"),
]
profiles = [
    ("pwc-core-read-v1", "Mandatory", CORE + " §5.1"),
    ("pwc-psbt-signer-v1", "Optional", CORE + " §5.2"),
]
protocols = [
    ("pwc-local/1", LOCAL, "Value of `local.hello` `protocols` and `protocol`"),
    ("pwc-browser/1", BROWSER, "Value of `PwcProviderInfo.protocol`"),
    ("pwc-remote/1", REMOTE + "; used unchanged by " + MOBILE, "Version `v=1` of the `pwc-remote:` pairing URI"),
]
networks = [("mainnet", "Existing Purity main network"), ("signet", "Existing Purity signet"),
            ("regtest", "Existing Purity regression-test network")]
script_types = ["p2pkh", "p2sh", "p2sh-p2wpkh", "p2sh-p2wsh", "p2wpkh", "p2wsh", "p2tr", "other"]
message_formats = ["bip322-simple", "bip322-full", "legacy"]
identifiers = [
    ("JSON-RPC error code `1000`", "Carries every `PwcError` in `error.data`", LOCAL + " §4.3"),
    ("JSON-RPC error codes `-32700`, `-32600`", "Parse error; invalid request", LOCAL + " §4.3"),
    ("Endpoint directory `pwc`", "Unix socket directory name under the platform base path", LOCAL + " §3.2"),
    ("Pipe prefix `pwc-<sid>-`", "Windows named pipe prefix", LOCAL + " §3.3"),
    ("HMAC labels `PWC-LOCAL-1 client`, `PWC-LOCAL-1 provider`", "Authentication proofs", LOCAL + " §5.4"),
    ("DOM events `pwc:requestProvider`, `pwc:announceProvider`", "Provider discovery", BROWSER + " §4.1"),
    ("Event type `pwcevent`", "Core events on the page interface", BROWSER + " §4.3"),
    ("Error name `PwcError`", "`name` of a rejected request's `Error`", BROWSER + " §4.2"),
    ("URI scheme `pwc-remote`", "Pairing URI", REMOTE + " §4.1"),
    ("Nostr event kind `27180`", "PWC messages (ephemeral; registration pending)", REMOTE + " §5.1"),
    ("Hash label `PWC-REMOTE-1 sas`", "Short authentication string", REMOTE + " §4.4"),
    ("Link parameter `pwc_session`", "Session reference in wake and return links", MOBILE + " §3"),
]
provider_ids = [("puritywallet", "PurityWallet desktop and its future mobile and extension editions")]

# Consistency with the Core schema
schema = json.loads((HERE / "pwc-core-0.2.1.schema.json").read_text(encoding="utf-8"))
core_methods = {m for m, *_ in methods if not m.startswith(("local.", "remote."))}
assert core_methods == set(schema["x-pwc-methods"]), core_methods ^ set(schema["x-pwc-methods"])
assert {s for s, *_ in scopes} == set(schema["$defs"]["Scope"]["anyOf"][0]["enum"])
assert {e for e, *_ in errors} == set(schema["$defs"]["ErrorCode"]["anyOf"][0]["enum"])
assert {e for e, *_ in events} == set(schema["$defs"]["EventType"]["anyOf"][0]["enum"])
assert {f for f, _ in features} == set(schema["$defs"]["Feature"]["anyOf"][0]["enum"])
for m in schema["x-pwc-methods"]:
    op = "operationResult" in schema["x-pwc-methods"][m]
    assert op == next(x[2] for x in methods if x[0] == m), m
    has_key = "idempotency_key" in schema["$defs"][f"{m}.params"]["properties"]
    assert has_key == next(x[3] for x in methods if x[0] == m), m

registry = {
    "registry_version": "0.1.0",
    "covers": [CORE, LOCAL, BROWSER, REMOTE, MOBILE],
    "methods": [{"name": n, "scope": s, "returns_operation": o, "idempotency_key": k,
                 "pwc_core_read_v1": n in READ_V1, "pwc_psbt_signer_v1": n == "psbt.sign", "defined_in": d}
                for n, s, o, k, d in methods],
    "notifications": [{"name": n, "direction": dr, "defined_in": d} for n, dr, d in notifications],
    "scopes": [{"name": s, "grants": g, "approval": a} for s, g, a in scopes],
    "features": [{"name": f, "meaning": m} for f, m in features],
    "errors": [{"code": c, "meaning": m,
                "raised_by": "transport component" if c == "PROVIDER_UNAVAILABLE" else "Provider"}
               for c, m in errors],
    "events": [{"event_type": e, "envelope": env, "payload": pl} for e, env, pl in events],
    "profiles": [{"name": p, "status": s, "defined_in": d} for p, s, d in profiles],
    "protocols": [{"name": p, "defined_in": d, "use": u} for p, d, u in protocols],
    "networks": [{"name": n, "meaning": m} for n, m in networks],
    "script_types": script_types,
    "message_formats": message_formats,
    "identifiers": [{"identifier": i, "use": u, "defined_in": d} for i, u, d in identifiers],
    "provider_ids": [{"provider_id": p, "holder": h} for p, h in provider_ids],
    "extension_namespace": "vendor.<reverse-domain>.<name>",
}
(HERE / "pwc-registry-0.1.0.json").write_text(json.dumps(registry, indent=2, ensure_ascii=False) + "\n",
                                              encoding="utf-8")


def table(header, body):
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines += ["| " + " | ".join(r) + " |" for r in body]
    return "\n".join(lines)


yes = lambda b: "Yes" if b else "—"
BR = "  "   # Markdown line break in the header block
md = f"""# Purity Wallet Connect (PWC)
## Registry of Names and Identifiers

**Specification:** PWC-REGISTRY-001{BR}
**Version:** 0.1.0 (review draft){BR}
**Date:** 2026-11-21{BR}
**Language:** English{BR}
**License:** CC BY 4.0{BR}
**Covers:** {CORE}; {LOCAL}; {BROWSER}; {REMOTE}; {MOBILE}{BR}
**Status:** Proposed; maintained under PWC-GOVERNANCE-001

> **Scope rule:** This registry lists every name and identifier that the PWC specifications define, so that implementations and extensions do not collide. It defines no behavior: each entry points to the section that does. A machine-readable copy is published alongside as `pwc-registry-0.1.0.json`; the two are generated from the same source.

## 1. How names are allocated

| Namespace | Who allocates | Procedure |
|---|---|---|
| Core and transport-profile names (`category.action` methods, scopes, features, error codes, event types, protocols, identifiers) | The specification maintainers | Only through a revision of the defining specification (PWC-GOVERNANCE-001 §4) |
| Extension names `vendor.<reverse-domain>.<name>` | Whoever controls the domain | No registration needed. The domain owner guarantees uniqueness (Core §11) |
| `provider_id` values (LOCAL §3.1, BROWSER §4.1, REMOTE §4.3) | The specification maintainers | First come, first served on request (PWC-GOVERNANCE-001 §5) |
| Network names | Fixed by the existing Purity networks | Only through a Core revision (Core §3.2) |

## 2. Methods

"Op" marks methods that return an Operation; "Key" marks methods that require an `idempotency_key` (Core §9.3).

{table(["Method", "Scope", "Op", "Key", "Profile", "Defined in"],
       [[f"`{n}`", f"`{s}`" if s else "none", yes(o), yes(k),
         "`pwc-core-read-v1`" if n in READ_V1 else ("`pwc-psbt-signer-v1`" if n == "psbt.sign" else "—"), d]
        for n, s, o, k, d in methods])}

The `local.*` methods exist only on LOCAL connections and the `remote.*` methods only on REMOTE pairings; they are never listed in `Capabilities.methods`.

## 3. Notifications

{table(["Notification", "Direction", "Defined in"], [[f"`{n}`", dr, d] for n, dr, d in notifications])}

## 4. Scopes

{table(["Scope", "Grants access to", "Approval behavior"], [[f"`{s}`", g, a] for s, g, a in scopes])}

## 5. Features

{table(["Feature", "Meaning"], [[f"`{f}`", m] for f, m in features])}

## 6. Error codes

Every code is a `PwcError.code` (Core §10). `PROVIDER_UNAVAILABLE` is generated by a transport component and never by a Provider; all others come from the Provider.

{table(["Code", "Meaning"], [[f"`{c}`", m] for c, m in errors])}

## 7. Event types

{table(["Event type", "Envelope fields", "Payload"], [[f"`{e}`", env, pl] for e, env, pl in events])}

## 8. Profiles and protocols

{table(["Profile", "Status", "Defined in"], [[f"`{p}`", s, d] for p, s, d in profiles])}

{table(["Protocol identifier", "Defined in", "Use"], [[f"`{p}`", d, u] for p, d, u in protocols])}

## 9. Networks, script types and message formats

{table(["Network", "Meaning"], [[f"`{n}`", m] for n, m in networks])}

Script types (Core §3.2): {", ".join(f"`{s}`" for s in script_types)}.

Message formats (Core §3.2, §8.4): {", ".join(f"`{f}`" for f in message_formats)}.

## 10. Transport identifiers

{table(["Identifier", "Use", "Defined in"], [[i, u, d] for i, u, d in identifiers])}

## 11. Provider identifiers

{table(["`provider_id`", "Holder"], [[f"`{p}`", h] for p, h in provider_ids])}

## 12. References

- {CORE}
- {LOCAL}
- {BROWSER}
- {REMOTE}
- {MOBILE}
- PWC-GOVERNANCE-001 v0.1.0

---

**End of PWC-REGISTRY-001 v0.1.0.**
"""
(out_dir / "PWC-Registry-v0.1.0-EN.md").write_text(md, encoding="utf-8")
print(f"registry: {len(methods)} methods, {len(notifications)} notifications, {len(scopes)} scopes, "
      f"{len(errors)} errors, {len(events)} events")
