"""Check the PWC test vectors with independent reference computations (PWC-CONFORMANCE-001 §5.1).

Usage: python3 run_vectors.py
Implementations use the same vector files to test their own code; this script proves the vectors are right.
"""
import hashlib
import hmac
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from balance_reference import compute   # noqa: E402

failures, count = [], 0


def check(vid, got, want):
    global count
    count += 1
    if got != want:
        failures.append(f"{vid}: got {got!r}, expected {want!r}")


misc = json.load(open(os.path.join(HERE, "vectors", "misc.json"), encoding="utf-8"))
for v in misc["fees"]["vectors"]:
    r, n = int(v["sat_per_kvb"]), v["vsize"]
    q, rem = divmod(r * n, 1000)
    check(v["id"], str(q + (1 if rem else 0)), v["fee_sats"])
for v in misc["local_auth"]["vectors"]:
    k, sn, cn = (bytes.fromhex(v[x]) for x in ("client_secret", "server_nonce", "client_nonce"))
    cid = v["client_id"].encode()
    check(v["id"] + "/client", hmac.new(k, b"PWC-LOCAL-1 client\x00" + sn + cn + cid, hashlib.sha256).hexdigest(),
          v["client_proof"])
    check(v["id"] + "/provider", hmac.new(k, b"PWC-LOCAL-1 provider\x00" + cn + sn + cid, hashlib.sha256).hexdigest(),
          v["server_proof"])
for v in misc["remote_sas"]["vectors"]:
    h = hashlib.sha256(b"PWC-REMOTE-1 sas\x00" + bytes.fromhex(v["client_key"]) + bytes.fromhex(v["provider_key"])
                       + bytes.fromhex(v["secret"])).digest()
    check(v["id"] + "/hash", h.hex(), v["hash"])
    check(v["id"] + "/sas", f"{int.from_bytes(h[:4], 'big') % 1_000_000:06d}", v["sas"])
for v in misc["remote_split"]["vectors"]:
    n = v["message_bytes"]
    check(v["id"], 1 if n <= 8192 else math.ceil(n / 6000), v["parts"])

balance = json.load(open(os.path.join(HERE, "vectors", "balance.json"), encoding="utf-8"))
for v in balance["vectors"]:
    check(v["id"], compute(v["scenario"]), v["expected"])

if failures:
    print("\n".join(failures))
    sys.exit(1)
print(f"all {count} vector checks passed")
