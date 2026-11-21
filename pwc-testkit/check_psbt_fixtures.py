"""Verify vectors/psbt.json independently of how it was produced.

Usage: python3 check_psbt_fixtures.py <path-to-embit/src>
Checks every signature against the BIP-143 sighash, the corrupted signature's failure, the version-2 fixture's
version field, the combined PSBT as the union of both partial PSBTs, and the finalized transaction's txid.
"""
import base64
import json
import os
import sys

sys.path.insert(0, sys.argv[1])
from embit import ec                                    # noqa: E402
from embit.psbt import PSBT                             # noqa: E402
from embit.transaction import Transaction               # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
fx = json.load(open(os.path.join(HERE, "vectors", "psbt.json"), encoding="utf-8"))
P = {k: v for k, v in fx["psbts"].items()}
failures = []


def sigs_ok(name):
    p = PSBT.from_string(P[name])
    inp = p.inputs[0]
    out = []
    for pub, sig in inp.partial_sigs.items():
        h = p.tx.sighash_segwit(0, inp.witness_script, inp.witness_utxo.value, sighash=sig[-1])
        out.append(pub.verify(ec.Signature.parse(sig[:-1]), h))
    return out


for name, expected in [("partial_k1", [True]), ("partial_k2", [True]), ("partial_k1_anyonecanpay", [True]),
                       ("combined", [True, True]), ("other_transaction_k2", [True]),
                       ("partial_k1_corrupted", [False])]:
    try:
        got = sigs_ok(name)
    except Exception as e:                              # a corrupted DER encoding may fail to parse
        got = [False] if name == "partial_k1_corrupted" else [repr(e)]
    if got != expected:
        failures.append(f"{name}: signature checks {got}, expected {expected}")

a, b, c = (PSBT.from_string(P[n]).inputs[0].partial_sigs for n in ("partial_k1", "partial_k2", "combined"))
if dict(c) != {**a, **b}:
    failures.append("combined is not the union of partial_k1 and partial_k2")
if PSBT.from_string(P["partial_k1"]).tx.txid() != PSBT.from_string(P["other_transaction_k2"]).tx.txid():
    pass
else:
    failures.append("other_transaction_k2 should be a different transaction")
acp = PSBT.from_string(P["partial_k1_anyonecanpay"]).inputs[0].partial_sigs
if list(acp.values())[0] == list(a.values())[0]:
    failures.append("anyonecanpay signature should differ from the default one")

raw = base64.b64decode(P["version2"])
if b"\x01\xfb\x04\x02\x00\x00\x00" not in raw or b"\x01\x00" in raw[5:7]:
    failures.append("version2 fixture lacks PSBT_GLOBAL_VERSION = 2 or carries an unsigned transaction")

tx = Transaction.parse(bytes.fromhex(fx["finalized"]["raw_transaction"]))
if tx.txid().hex() != fx["finalized"]["txid"]:
    failures.append("finalized txid mismatch")
if tx.txid() != PSBT.from_string(P["combined"]).tx.txid():
    failures.append("finalized transaction differs from the combined PSBT's transaction")
stack = tx.vin[0].witness.items
if len(stack) != 4 or stack[0] != b"":
    failures.append(f"unexpected witness stack of {len(stack)} items")

if failures:
    print("\n".join(failures))
    sys.exit(1)
print("psbt fixtures verified: 6 signature sets, union, distinct transaction, version 2 field, final transaction")
