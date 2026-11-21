"""Validate the PWC schemas against the examples in the specifications and against negative cases.

Usage: python3 check_schema.py <PWC-Core.md> <PWC-Transport-Local.md> <PWC-Transport-Remote.md>
Exit status 0 when every check passes.
"""
import json
import pathlib
import re
import sys

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

HERE = pathlib.Path(__file__).resolve().parent
core = json.loads((HERE / "pwc-core-0.2.1.schema.json").read_text(encoding="utf-8"))
strict = json.loads((HERE / "pwc-core-0.2.1.strict.schema.json").read_text(encoding="utf-8"))
local = json.loads((HERE / "pwc-transport-local-0.1.0.schema.json").read_text(encoding="utf-8"))
browser = json.loads((HERE / "pwc-transport-browser-0.1.0.schema.json").read_text(encoding="utf-8"))
remote = json.loads((HERE / "pwc-transport-remote-0.1.0.schema.json").read_text(encoding="utf-8"))
ALL = (core, strict, local, browser, remote)
registry = Registry().with_resources([(s["$id"], Resource.from_contents(s)) for s in ALL])

for s in ALL:
    Draft202012Validator.check_schema(s)

PLACEHOLDERS = {
    "base64-psbt": "cHNidP8=", "updated-base64-psbt": "cHNidP8=", "merged-base64-psbt": "cHNidP8=",
    "partial-A": "cHNidP8=", "partial-B": "cHNidP8=", "existing-purity-address": "bc1qexampleaddress",
    "64 hex": "ab" * 32,
}
failures = []


def fill(text):
    return re.sub(r'"<([^>]+)>"', lambda m: json.dumps(PLACEHOLDERS[m.group(1)]), text)


def validator(schema, name):
    return Draft202012Validator({"$ref": f"{schema['$id']}#/$defs/{name}"}, registry=registry,
                                format_checker=FormatChecker())


def expect(schema, name, instance, valid, label):
    errors = list(validator(schema, name).iter_errors(instance))
    ok = (not errors) if valid else bool(errors)
    if not ok:
        detail = errors[0].message if errors else "unexpectedly valid"
        failures.append(f"{label}: expected {'valid' if valid else 'invalid'} for {name}: {detail}")


# 1. Core §14 examples, in document order
core_md = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
sec14 = core_md[core_md.index("## 14."):core_md.index("## 15.")]
blocks = [json.loads(fill(b)) for b in re.findall(r"```json\n(.*?)```", sec14, re.S)]
expected = ["session.requestPermissions", "Operation", "account.getBalance", "Balance",
            "psbt.create", "psbt.create.operationResult", "psbt.sign", "psbt.sign.operationResult",
            "psbt.combine", "psbt.combine.result"]
if len(blocks) != len(expected):
    failures.append(f"Core §14: found {len(blocks)} JSON blocks, expected {len(expected)}")
for i, (block, name) in enumerate(zip(blocks, expected)):
    label = f"Core §14 block {i + 1}"
    if "params" in block:
        if block["method"] != name:
            failures.append(f"{label}: method {block['method']} != {name}")
        for s in (core, strict):
            expect(s, f"{name}.params", block["params"], True, label)
    else:
        for s in (core, strict):
            expect(s, name, block, True, label)

# 2. LOCAL §12 frames
local_md = pathlib.Path(sys.argv[2]).read_text(encoding="utf-8")
sec12 = local_md[local_md.index("## 12."):local_md.index("## 13.")]
frames = 0
for line in sec12.splitlines():
    m = re.match(r"^(C→P|P→C) (\{.*\})$", line)
    if not m or "…" in line:
        continue
    frames += 1
    frame = json.loads(fill(m.group(2)))
    label = f"LOCAL §12 frame {frames} ({m.group(1)})"
    if m.group(1) == "C→P":
        expect(local, "Request", frame, True, label)
        method = frame["method"]
        target = local if method.startswith("local.") else core
        expect(target, f"{method}.params", frame["params"], True, label)
    else:
        expect(local, "ProviderFrame", frame, True, label)
if frames < 8:
    failures.append(f"LOCAL §12: only {frames} complete frames found")

# 3. Negative and boundary cases (Core §3.2, §13)
TYPES = pathlib.Path(__file__).resolve().parent.parent / "pwc-testkit" / "vectors" / "types.json"
cases = [(c["type"], c["value"], c["valid"]) for c in json.loads(TYPES.read_text(encoding="utf-8"))["vectors"]]
for name, value, valid in cases:
    expect(core, name, value, valid, f"case {name}={str(value)[:40]}")

# Strict variant rejects unknown members; permissive accepts them (Core §3.1, C-28)
balance = blocks[3]
leaked = dict(balance, xpub="xpub-internal")
expect(core, "Balance", leaked, True, "permissive accepts unknown member")
expect(strict, "Balance", leaked, False, "strict rejects unknown member")

# LOCAL negative frames
expect(local, "ErrorResponse", {"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "x"}}, False,
       "LOCAL -32601 not used")
expect(local, "Request", {"jsonrpc": "2.0", "id": None, "method": "wallet.list", "params": {}}, False,
       "LOCAL null id")
expect(local, "Request", {"jsonrpc": "2.0", "id": 1, "method": "wallet.list"}, False, "LOCAL params required")
expect(local, "local.pair.params", {"client_name": "X", "pairing_code": "12345"}, False, "LOCAL 5-digit code")

# REMOTE §12.3 plaintexts and envelope cases
remote_md = pathlib.Path(sys.argv[3]).read_text(encoding="utf-8")
sec12r = remote_md[remote_md.index("## 12."):remote_md.index("## 13.")]
rblocks = [json.loads(b) for b in re.findall(r"```json\n(.*?)```", sec12r, re.S)]
if len(rblocks) != 2:
    failures.append(f"REMOTE §12: found {len(rblocks)} JSON blocks, expected 2")
for i, b in enumerate(rblocks):
    expect(remote, "Plaintext", b, True, f"REMOTE §12.3 block {i + 1}")
event = {"id": "aa" * 32, "pubkey": "11" * 32, "created_at": 1763683200, "kind": 27180,
         "tags": [["p", "22" * 32]], "content": "AgAB", "sig": "bb" * 64}
expect(remote, "NostrEvent", event, True, "REMOTE event")
expect(remote, "NostrEvent", dict(event, kind=24133), False, "REMOTE wrong kind")
expect(remote, "NostrEvent", dict(event, tags=[["p", "22" * 32], ["p", "33" * 32]]), False, "REMOTE two p tags")
part = {"pwc_part": {"message_id": "cd" * 16, "index": 0, "count": 4, "data": "cHNidP8="}}
expect(remote, "Plaintext", part, True, "REMOTE part")
expect(remote, "RemotePart", {"pwc_part": dict(part["pwc_part"], count=1)}, False, "REMOTE single-part count")
paired = {"jsonrpc": "2.0", "method": "remote.paired", "params": {
    "secret": "00112233445566778899aabbccddeeff", "client_id": "cl-1", "session_id": "ss-41",
    "pwc_version": "0.2.1", "provider_id": "puritywallet", "provider": {"name": "PurityWallet", "version": "1.1.1"},
    "relays": ["wss://relay.example.com"], "max_message_bytes": 1048576, "wake_url": "https://wallet.example/pwc-wake"}}
expect(remote, "Plaintext", paired, True, "REMOTE remote.paired")
bad = json.loads(json.dumps(paired)); bad["params"]["relays"] = ["ws://relay.example.com"]
expect(remote, "Plaintext", bad, False, "REMOTE non-wss relay")
expect(remote, "remote.updateRelays.params", {"relays": []}, False, "REMOTE empty relays")

# BROWSER values
info = {"uuid": "6f1c2a9e-4b7d-4c3a-9e2f-1a2b3c4d5e6f", "provider_id": "puritywallet", "name": "PurityWallet",
        "icon": "data:image/svg+xml;base64,PHN2Zy8+", "protocol": "pwc-browser/1"}
expect(browser, "PwcProviderInfo", info, True, "BROWSER info")
expect(browser, "PwcProviderInfo", dict(info, icon="https://wallet.example/icon.png"), False, "BROWSER remote icon")
expect(browser, "RejectionError", {"name": "PwcError", "code": "PROVIDER_UNAVAILABLE", "message": "x"}, True,
       "BROWSER rejection")
expect(browser, "RequestArguments", {"method": "session.requestPermissions",
                                     "params": {"scopes": ["wallets:read"], "idempotency_key": "6f1c2a9e-4b7d-4c3a-9e2f-1a2b3c4d5e6f"}},
       True, "BROWSER request with UUID key")

if failures:
    print("\n".join(failures))
    sys.exit(1)
print(f"all checks passed: {len(blocks)} Core examples, {frames} LOCAL frames, {len(rblocks)} REMOTE examples, "
      f"{len(cases) + 6} Core/LOCAL cases, 12 REMOTE/BROWSER cases")
