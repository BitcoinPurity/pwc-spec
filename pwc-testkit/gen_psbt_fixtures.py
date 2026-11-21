"""Generate the PSBT fixtures of PWC-CONFORMANCE-001 §5.1 (vectors/psbt.json).

Requires the embit library (https://github.com/diybitcoinhardware/embit):
  python3 gen_psbt_fixtures.py <path-to-embit/src>
Keys are fixed test keys, never to be used with real funds. Network: regtest.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, sys.argv[1])
from embit import ec, script                                          # noqa: E402
from embit.finalizer import finalize_psbt                             # noqa: E402
from embit.psbt import PSBT, InputScope, OutputScope                  # noqa: E402
from embit.transaction import Transaction, TransactionInput, TransactionOutput, SIGHASH  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))


def key(label):
    return ec.PrivateKey(hashlib.sha256(b"PWC test key " + label).digest())


k1, k2, k3 = key(b"1"), key(b"2"), key(b"3")
ws = script.multisig(2, [k1.get_public_key(), k2.get_public_key()])
spk_in = script.p2wsh(ws)
spk_out = script.p2wpkh(k3.get_public_key())
prev_txid = bytes.fromhex("aa" * 32)


def unsigned(amount_out):
    tx = Transaction(vin=[TransactionInput(prev_txid, 0)], vout=[TransactionOutput(amount_out, spk_out)])
    p = PSBT(tx)
    p.inputs[0].witness_utxo = TransactionOutput(100_000, spk_in)
    p.inputs[0].witness_script = ws
    return p


def b64(p):
    return p.to_string()


def signed(amount_out, priv, sighash=SIGHASH.ALL):
    p = unsigned(amount_out)
    n = p.sign_with(priv, sighash=sighash)
    assert n == 1, n
    return p


base = unsigned(99_000)
part_a = signed(99_000, k1)
part_b = signed(99_000, k2)
part_a_acp = signed(99_000, k1, SIGHASH.ALL | SIGHASH.ANYONECANPAY)
other_tx = signed(98_000, k2)

combined = PSBT.from_string(b64(part_a))
combined.inputs[0].partial_sigs.update(part_b.inputs[0].partial_sigs)
assert len(combined.inputs[0].partial_sigs) == 2

final_tx = finalize_psbt(PSBT.from_string(b64(combined)))
assert final_tx is not None
raw = final_tx.serialize().hex()
txid = final_tx.txid().hex()

# A corrupted signature: same key, a value that does not verify
bad = PSBT.from_string(b64(part_a))
pub = next(iter(bad.inputs[0].partial_sigs))
sig = bytearray(bad.inputs[0].partial_sigs[pub])
sig[-2] ^= 0x01
bad.inputs[0].partial_sigs[pub] = bytes(sig)


def compact(n):
    return bytes([n]) if n < 0xfd else b"\xfd" + n.to_bytes(2, "little")


def kv(k, v):
    return compact(len(k)) + k + compact(len(v)) + v


# A minimal, well-formed BIP-370 version 2 PSBT for rejection tests
witness_utxo = (100_000).to_bytes(8, "little") + compact(len(spk_in.data)) + spk_in.data
v2 = (b"psbt\xff"
      + kv(b"\x02", (2).to_bytes(4, "little"))          # PSBT_GLOBAL_TX_VERSION
      + kv(b"\x04", compact(1))                         # PSBT_GLOBAL_INPUT_COUNT
      + kv(b"\x05", compact(1))                         # PSBT_GLOBAL_OUTPUT_COUNT
      + kv(b"\xfb", (2).to_bytes(4, "little"))          # PSBT_GLOBAL_VERSION
      + b"\x00"
      + kv(b"\x0e", prev_txid) + kv(b"\x0f", (0).to_bytes(4, "little")) + kv(b"\x01", witness_utxo)
      + b"\x00"
      + kv(b"\x03", (99_000).to_bytes(8, "little")) + kv(b"\x04", spk_out.data)
      + b"\x00")
import base64  # noqa: E402

fixtures = {
    "description": ("PSBT fixtures for PWC-CORE-001 v0.2.1 §4.7 and §8.3, covering Core tests C-11 to C-14, "
                    "C-40 and C-55. 2-of-2 P2WSH input of 100,000 sat on regtest, one "
                    "output of 99,000 sat, fee 1,000 sat. Compare PSBTs by their sets of key-value pairs, not by "
                    "bytes, because serializations may order keys differently."),
    "keys": {"k1": k1.get_public_key().sec().hex(), "k2": k2.get_public_key().sec().hex(),
             "note": "Private keys are SHA-256('PWC test key 1') and SHA-256('PWC test key 2'). Test only."},
    "psbts": {
        "unsigned": b64(base),
        "partial_k1": b64(part_a),
        "partial_k2": b64(part_b),
        "partial_k1_anyonecanpay": b64(part_a_acp),
        "partial_k1_corrupted": b64(bad),
        "other_transaction_k2": b64(other_tx),
        "combined": b64(combined),
        "version2": base64.b64encode(v2).decode(),
    },
    "finalized": {"raw_transaction": raw, "txid": txid},
    "cases": [
        {"id": "PSBT-01", "method": "psbt.combine", "inputs": ["partial_k1", "partial_k2"],
         "expect": {"result_equals": "combined"}},
        {"id": "PSBT-02", "method": "psbt.combine", "inputs": ["partial_k1", "other_transaction_k2"],
         "expect": {"error": "PSBT_CONFLICT"}},
        {"id": "PSBT-03", "method": "psbt.combine", "inputs": ["partial_k1", "version2"],
         "expect": {"error": "UNSUPPORTED_PSBT_VERSION"}},
        {"id": "PSBT-04", "method": "psbt.combine", "inputs": ["partial_k1", "partial_k1_anyonecanpay"],
         "expect": {"result_equals": "partial_k1",
                    "note": "both signatures verify; the first submitted value is kept"}},
        {"id": "PSBT-05", "method": "psbt.combine", "inputs": ["partial_k1", "partial_k1_corrupted"],
         "expect": {"error": "PSBT_CONFLICT"}},
        {"id": "PSBT-06", "method": "psbt.finalize", "inputs": ["partial_k1"],
         "expect": {"complete": False}},
        {"id": "PSBT-07", "method": "psbt.finalize", "inputs": ["combined"],
         "expect": {"complete": True, "raw_transaction": raw, "txid": txid}},
        {"id": "PSBT-08", "method": "psbt.finalize", "inputs": ["version2"],
         "expect": {"error": "UNSUPPORTED_PSBT_VERSION"}},
    ],
}
with open(os.path.join(HERE, "vectors", "psbt.json"), "w", encoding="utf-8") as out:
    json.dump(fixtures, out, indent=2)
print("psbt fixtures written; txid", txid)
