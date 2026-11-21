"""PWC LOCAL conformance runner (PWC-CONFORMANCE-001 §5.2).

Runs the automatable LOCAL tests of PWC-TRANSPORT-LOCAL-001 v0.1.0 §11 against a Provider endpoint and writes a
JSON report in the format of PWC-CONFORMANCE-001 §6. Tests that need a person, another operating-system user or
platform inspection are reported as "manual".

Usage:
  python3 local_runner.py --socket PATH [--pair] [--allow-destructive] [--report out.json]
--pair             pair a test Client first (the Provider will prompt the User; approve it)
--allow-destructive  also run L-23 (local.unpair), which deletes the test pairing
"""
import argparse
import json
import os
import secrets
import socket
import stat
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pwc_local_client import LocalClient, PwcError, client_proof, server_proof   # noqa: E402

results = []


def record(test_id, outcome, notes=""):
    results.append({"id": test_id, "outcome": outcome, "notes": notes})
    print(f"{test_id:6} {outcome:7} {notes}")


def raw(path):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(10)
    s.connect(path)
    return s, s.makefile("rb")


def send(s, obj):
    s.sendall(json.dumps(obj, separators=(",", ":")).encode() + b"\n")


def recv(f):
    line = f.readline()
    return json.loads(line) if line else None


def closed(s, f):
    try:
        return f.readline() == b""
    except (ConnectionError, socket.timeout):
        return True


def pwc_code(msg):
    try:
        return msg["error"]["data"]["code"]
    except (TypeError, KeyError):
        return None


def hello(s, f):
    send(s, {"jsonrpc": "2.0", "id": "h", "method": "local.hello", "params": {"protocols": ["pwc-local/1"]}})
    return recv(f)["result"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--socket", required=True)
    ap.add_argument("--pair", action="store_true")
    ap.add_argument("--allow-destructive", action="store_true")
    ap.add_argument("--report")
    args = ap.parse_args()
    path = args.socket

    # L-01 (passive part): permissions of the endpoint as found
    st_dir, st_sock = os.stat(os.path.dirname(path)), os.stat(path)
    ok = (st_dir.st_uid == os.getuid() and not stat.S_IMODE(st_dir.st_mode) & 0o077
          and not stat.S_IMODE(st_sock.st_mode) & 0o077)
    record("L-01", "pass" if ok else "fail", "endpoint directory 0700 and socket without group/other access"
           " (refusal to start on a bad directory is checked manually)")
    record("L-02", "manual", "needs a second operating-system user")

    s, f = raw(path)
    send(s, {"jsonrpc": "2.0", "id": 1, "method": "pwc.getCapabilities", "params": {}})
    m = recv(f)
    record("L-03", "pass" if pwc_code(m) == "UNAUTHENTICATED" and closed(s, f) else "fail")
    s.close()

    s, f = raw(path)
    hello(s, f)
    send(s, {"jsonrpc": "2.0", "id": 2, "method": "wallet.list", "params": {}})
    record("L-04", "pass" if pwc_code(recv(f)) == "UNAUTHENTICATED" else "fail")
    s.close()

    s, f = raw(path)
    send(s, {"jsonrpc": "2.0", "id": 1, "method": "local.hello", "params": {"protocols": ["pwc-local/99"]}})
    m = recv(f)
    record("L-05", "pass" if pwc_code(m) == "UNSUPPORTED_OPERATION" and closed(s, f) else "fail")
    s.close()

    credential = None
    if args.pair:
        c = LocalClient(path)
        c.say_hello()
        try:
            r = c.pair("PWC conformance runner")
            ok = len(r["client_secret"]) == 64 and "session_id" in r
            c.request("session.getInfo")
            record("L-06", "pass" if ok else "fail")
            credential = (r["client_id"], r["client_secret"])
        except PwcError as e:
            record("L-06", "fail", e.code)
        c.close()
    else:
        record("L-06", "skip", "run with --pair")
    for tid in ("L-07", "L-08", "L-09"):
        record(tid, "manual", "needs the User to refuse, ignore or abandon a pairing prompt")

    # L-10: wrong proof versus unknown client_id
    def failed_auth(client_id, secret_hex):
        s, f = raw(path)
        h = hello(s, f)
        cn = secrets.token_bytes(32)
        proof = client_proof(bytes.fromhex(secret_hex), bytes.fromhex(h["server_nonce"]), cn, client_id)
        send(s, {"jsonrpc": "2.0", "id": 3, "method": "local.authenticate",
                 "params": {"client_id": client_id, "client_nonce": cn.hex(), "client_proof": proof.hex()}})
        m = recv(f)
        was_closed = closed(s, f)
        s.close()
        return m, was_closed

    if credential:
        m1, c1 = failed_auth(credential[0], "00" * 32)
        m2, c2 = failed_auth("cl-unknown-" + secrets.token_hex(4), "00" * 32)
        same = m1 and m2 and m1["error"] == m2["error"] and pwc_code(m1) == "UNAUTHENTICATED"
        record("L-10", "pass" if same and c1 and c2 else "fail")

        # L-11: replay a proof from an earlier connection
        s, f = raw(path)
        h = hello(s, f)
        cn = secrets.token_bytes(32)
        old = client_proof(bytes.fromhex(credential[1]), bytes.fromhex(h["server_nonce"]), cn, credential[0])
        send(s, {"jsonrpc": "2.0", "id": 4, "method": "local.authenticate",
                 "params": {"client_id": credential[0], "client_nonce": cn.hex(), "client_proof": old.hex()}})
        first_ok = "result" in recv(f)
        s.close()
        s, f = raw(path)
        hello(s, f)
        send(s, {"jsonrpc": "2.0", "id": 5, "method": "local.authenticate",
                 "params": {"client_id": credential[0], "client_nonce": cn.hex(), "client_proof": old.hex()}})
        record("L-11", "pass" if first_ok and pwc_code(recv(f)) == "UNAUTHENTICATED" else "fail")
        s.close()
    else:
        record("L-10", "skip", "needs a paired credential (--pair)")
        record("L-11", "skip", "needs a paired credential (--pair)")

    vec = (client_proof(bytes(range(32)), bytes(range(0x20, 0x40)), bytes(range(0x40, 0x60)), "cl-7Qm2xA").hex(),
           server_proof(bytes(range(32)), bytes(range(0x20, 0x40)), bytes(range(0x40, 0x60)), "cl-7Qm2xA").hex())
    record("L-12", "pass" if vec == ("3c8210fe06538455dd0d56d572cde9134c529c781a5e4e374f2a7cfcaccc1488",
                                     "a91ace84622f91fef391a014e55570ab722299a9af64aa25f2809c67decc29b2") else "fail",
           "runner's own HMAC implementation; the Provider's is exercised by L-06 and L-11")
    record("L-13", "manual", "Client-side test: run against a fake endpoint")

    s, f = raw(path)
    h = hello(s, f)
    big = b'{"jsonrpc":"2.0","id":9,"method":"local.pair","params":{"client_name":"' + \
          b"x" * h["max_frame_bytes"] + b'"}}\n'
    try:
        s.sendall(big)
    except (BrokenPipeError, ConnectionResetError):
        pass
    m = recv(f)
    record("L-14", "pass" if m and pwc_code(m) == "PAYLOAD_TOO_LARGE" and m.get("id") is None and closed(s, f)
           else "fail")
    s.close()

    s, f = raw(path)
    hello(s, f)
    s.sendall(b"{not json}\n")
    m = recv(f)
    send(s, {"jsonrpc": "2.0", "id": 10, "method": "wallet.list", "params": {}})
    record("L-15", "pass" if m["error"]["code"] == -32700 and pwc_code(recv(f)) == "UNAUTHENTICATED" else "fail")
    send(s, [{"jsonrpc": "2.0", "id": 11, "method": "wallet.list", "params": {}}])
    record("L-16", "pass" if recv(f)["error"]["code"] == -32600 else "fail")
    s.close()

    s, f = raw(path)
    hello(s, f)
    pair = {"jsonrpc": "2.0", "id": 12, "method": "local.pair",
            "params": {"client_name": "Duplicate id test", "pairing_code": "000000"}}
    send(s, pair)
    send(s, pair)
    first = recv(f)
    dup_rejected = first.get("error", {}).get("code") == -32600
    record("L-17", "pass" if dup_rejected else "fail",
           "a second frame with the id of a pending local.pair; the pairing prompt may stay open")
    s.close()

    if credential:
        c = LocalClient(path)
        c.say_hello()
        c.authenticate(*credential)
        m = c.call("vendor.example.nothing", {})
        record("L-18", "pass" if pwc_code(m) == "UNSUPPORTED_METHOD" and m["error"]["code"] == 1000 else "fail")
        m = c.call("wallet.list", {})
        err = m.get("error", {})
        ok = err.get("code") == 1000 and err.get("data", {}).get("code") in ("PERMISSION_REQUIRED",) \
            and err.get("message") == err["data"].get("message")
        record("L-19", "pass" if ok else "fail", "assumes the test Client holds no grant")
        key = "runner-" + secrets.token_hex(6)
        params = {"scopes": ["wallets:read"], "idempotency_key": key}
        op1 = c.request("session.requestPermissions", params)
        c.close()
        c = LocalClient(path)
        c.say_hello()
        c.authenticate(*credential)
        op2 = c.request("session.requestPermissions", params)
        record("L-20", "pass" if op1["operation_id"] == op2["operation_id"] else "fail",
               "reconnect and retry with the same idempotency key (session.requestPermissions)")
        if args.allow_destructive:
            c.request("local.unpair")
            time.sleep(0.2)
            m1, _ = failed_auth(credential[0], credential[1])
            record("L-23", "pass" if pwc_code(m1) == "UNAUTHENTICATED" else "fail")
        else:
            record("L-23", "skip", "run with --allow-destructive")
        c.close()
    else:
        for tid in ("L-18", "L-19", "L-20", "L-23"):
            record(tid, "skip", "needs a paired credential (--pair)")

    for tid, why in [("L-21", "needs a grant with events:read"), ("L-22", "needs a grant with events:read"),
                     ("L-24", "Windows only"), ("L-25", "platform inspection"), ("L-26", "restart the Provider"),
                     ("L-27", "visual inspection of the prompt"), ("L-28", "inspect listening sockets")]:
        record(tid, "manual", why)
    for tid in ("L-29", "L-30", "L-31", "L-32", "L-33", "L-34"):
        record(tid, "manual", "delegation tests; run with a delegator credential")

    report = {"suite": "PWC-TRANSPORT-LOCAL-001 v0.1.0 §11", "runner": "local_runner.py 0.1.0",
              "target": path, "date": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "results": results}
    if args.report:
        with open(args.report, "w", encoding="utf-8") as out:
            json.dump(report, out, indent=2)
    failed = [r for r in results if r["outcome"] == "fail"]
    print(f"\n{len(results)} tests: {sum(r['outcome'] == 'pass' for r in results)} pass, {len(failed)} fail")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
