"""Mock PWC LOCAL Provider for testing Clients and the LOCAL runner (Linux and macOS).

It implements the LOCAL handshake, framing and error mapping of PWC-TRANSPORT-LOCAL-001 v0.1.0 and a handful
of Core methods with fixed data. Pairing prompts are answered automatically after a short delay, standing in
for the User. It holds no wallet and signs nothing.

Usage: python3 mock_local_provider.py [--dir DIR] [--reject-pairing]
"""
import argparse
import asyncio
import datetime
import hashlib
import hmac
import json
import os
import secrets
import socket
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pwc_local_client import endpoint_directory  # noqa: E402

MAX_FRAME = 1_048_576
PROVIDER_ID = "mockwallet"


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def later(seconds):
    t = datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(seconds=seconds)
    return t.strftime("%Y-%m-%dT%H:%M:%SZ")


CAPABILITIES = {
    "pwc_version": "0.2.1",
    "provider": {"name": "Mock Wallet", "version": "0.1.0", "platform": "desktop"},
    "profiles": ["pwc-core-read-v1"],
    "supported_networks": ["regtest"],
    "methods": ["pwc.getCapabilities", "session.getInfo", "session.requestPermissions", "session.getPermissions",
                "session.revokePermissions", "wallet.list", "account.list", "account.getBalance",
                "address.getReceiveAddress", "transaction.list", "operation.get", "operation.cancel"],
    "features": [],
    "scopes": ["wallets:read", "balances:read", "addresses:read", "transactions:read"],
    "script_types": ["p2wpkh"],
    "message_formats": [],
    "limits": {"max_page_size": 100, "max_psbt_bytes": 100_000, "max_combine_psbts": 2,
               "max_raw_transaction_bytes": 100_000, "max_message_bytes": 1024, "max_concurrent_operations": 4,
               "operation_retention_seconds": 86_400, "high_fee_rate": "100000"},
}
SCOPED = {"wallet.list": "wallets:read", "account.list": "wallets:read", "account.getBalance": "balances:read",
          "address.getReceiveAddress": "addresses:read", "transaction.list": "transactions:read"}


def pwc_error(code, message):
    return {"code": 1000, "message": message, "data": {"code": code, "message": message}}


class Provider:
    def __init__(self, reject_pairing):
        self.reject_pairing = reject_pairing
        self.clients = {}           # client_id -> secret bytes
        self.operations = {}        # operation_id -> (client_id, operation)
        self.idempotency = {}       # (client_id, method, key) -> (params, response result)

    async def handle(self, reader, writer):
        sock = writer.get_extra_info("socket")
        if not self.same_user(sock):
            writer.close()
            return
        conn = {"stage": "new", "outstanding": set(), "writer": writer}
        try:
            line = await asyncio.wait_for(self.read_frame(reader, writer), timeout=10)
            while line is not None:
                await self.dispatch(conn, line)
                if conn["stage"] == "closed":
                    break
                timeout = 60 if conn["stage"] == "helloed" else None
                line = await asyncio.wait_for(self.read_frame(reader, writer), timeout=timeout)
        except (asyncio.TimeoutError, ConnectionError):
            pass
        finally:
            writer.close()

    @staticmethod
    def same_user(sock):
        if hasattr(socket, "SO_PEERCRED"):
            pid, uid, gid = struct.unpack("3i", sock.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED,
                                                                 struct.calcsize("3i")))
            return uid == os.getuid()
        return hasattr(os, "getuid")    # macOS: rely on directory permissions in this mock

    async def read_frame(self, reader, writer):
        try:
            line = await reader.readline()
        except (asyncio.LimitOverrunError, ValueError):
            await self.send(writer, {"jsonrpc": "2.0", "id": None,
                                     "error": pwc_error("PAYLOAD_TOO_LARGE", "Frame exceeds max_frame_bytes")})
            raise ConnectionError("oversized frame")
        if not line:
            return None
        return line

    @staticmethod
    async def send(writer, obj):
        writer.write(json.dumps(obj, separators=(",", ":")).encode() + b"\n")
        await writer.drain()

    async def dispatch(self, conn, line):
        writer = conn["writer"]
        try:
            msg = json.loads(line)
        except ValueError:
            await self.send(writer, {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}})
            return
        rid = msg.get("id") if isinstance(msg, dict) else None
        valid_id = (isinstance(rid, str) and 1 <= len(rid) <= 64) or (isinstance(rid, int) and not isinstance(rid, bool))
        if not isinstance(msg, dict) or msg.get("jsonrpc") != "2.0" or not isinstance(msg.get("method"), str):
            await self.send(writer, {"jsonrpc": "2.0", "id": None, "error": {"code": -32600, "message": "Invalid Request"}})
            return
        if "id" not in msg:
            return                                                         # notification: discarded
        if not valid_id or not isinstance(msg.get("params"), dict) or rid in conn["outstanding"]:
            await self.send(writer, {"jsonrpc": "2.0", "id": rid if valid_id else None,
                                     "error": {"code": -32600, "message": "Invalid Request"}})
            return
        conn["outstanding"].add(rid)
        asyncio.ensure_future(self.process(conn, rid, msg["method"], msg["params"]))

    async def process(self, conn, rid, method, params):
        writer = conn["writer"]
        try:
            result, close_after = await self.evaluate(conn, method, params)
            reply = {"jsonrpc": "2.0", "id": rid}
            reply.update({"error": result} if "code" in result and "data" in result else {"result": result})
            await self.send(writer, reply)
            if close_after:
                conn["stage"] = "closed"
                writer.close()
        finally:
            conn["outstanding"].discard(rid)

    async def evaluate(self, conn, method, params):
        stage = conn["stage"]
        if stage == "new":
            if method != "local.hello":
                return pwc_error("UNAUTHENTICATED", "local.hello must come first"), True
            if "pwc-local/1" not in params.get("protocols", []):
                return pwc_error("UNSUPPORTED_OPERATION", "No common protocol"), True
            conn["server_nonce"] = secrets.token_bytes(32)
            conn["stage"] = "helloed"
            return {"protocol": "pwc-local/1", "pwc_version": "0.2.1", "provider_id": PROVIDER_ID,
                    "provider": {"name": "Mock Wallet", "version": "0.1.0"}, "max_frame_bytes": MAX_FRAME,
                    "server_nonce": conn["server_nonce"].hex(), "delegation": False}, False
        if method == "local.hello":
            return pwc_error("INVALID_ARGUMENT", "local.hello already received"), False
        if stage == "helloed":
            if method == "local.pair":
                if conn.get("pairing"):
                    return pwc_error("RATE_LIMITED", "One pairing prompt per connection"), False
                conn["pairing"] = True
                await asyncio.sleep(0.5)                                   # the User reads the prompt
                if self.reject_pairing:
                    return pwc_error("USER_REJECTED", "Pairing refused"), False
                if params.get("delegator"):
                    return pwc_error("UNSUPPORTED_OPERATION", "Delegation not implemented"), False
                client_id = "cl-" + secrets.token_hex(6)
                secret = secrets.token_bytes(32)
                self.clients[client_id] = secret
                self.start_session(conn, client_id)
                return {"client_id": client_id, "client_secret": secret.hex(), "session_id": conn["session_id"]}, False
            if method == "local.authenticate":
                secret = self.clients.get(params.get("client_id"))
                try:
                    client_nonce = bytes.fromhex(params["client_nonce"])
                    proof = bytes.fromhex(params["client_proof"])
                except (KeyError, ValueError):
                    return pwc_error("INVALID_ARGUMENT", "Malformed authentication"), True
                cid = params.get("client_id", "")
                if secret is None:
                    secret = b"\x00" * 32                                  # same work, same answer
                expected = hmac.new(secret, b"PWC-LOCAL-1 client\x00" + conn["server_nonce"] + client_nonce
                                    + cid.encode(), hashlib.sha256).digest()
                if cid not in self.clients or not hmac.compare_digest(expected, proof):
                    await asyncio.sleep(0.2)
                    return pwc_error("UNAUTHENTICATED", "Authentication failed"), True
                self.start_session(conn, cid)
                server = hmac.new(secret, b"PWC-LOCAL-1 provider\x00" + client_nonce + conn["server_nonce"]
                                  + cid.encode(), hashlib.sha256).digest()
                return {"session_id": conn["session_id"], "server_proof": server.hex()}, False
            return pwc_error("UNAUTHENTICATED", "Not authenticated"), False
        return self.core(conn, method, params)

    def start_session(self, conn, client_id):
        conn.update(stage="authenticated", client_id=client_id, session_id="ss-" + secrets.token_hex(6),
                    created_at=now())

    def core(self, conn, method, params):
        cid = conn["client_id"]
        if method == "local.unpair":
            self.clients.pop(cid, None)
            return {"unpaired": True}, True
        if method.startswith("local.") or method not in CAPABILITIES["methods"]:
            return pwc_error("UNSUPPORTED_METHOD", f"Method {method} is not implemented"), False
        if method in SCOPED:
            return pwc_error("PERMISSION_REQUIRED", f"Scope {SCOPED[method]} is required"), False
        if method == "pwc.getCapabilities":
            return CAPABILITIES, False
        if method == "session.getInfo":
            return {"session_id": conn["session_id"], "client_id": cid, "grant_ids": [],
                    "created_at": conn["created_at"]}, False
        if method == "session.getPermissions":
            return {"grants": []}, False
        if method == "session.requestPermissions":
            key = params.get("idempotency_key")
            if not isinstance(key, str) or not isinstance(params.get("scopes"), list) or not params["scopes"]:
                return pwc_error("INVALID_ARGUMENT", "scopes and idempotency_key are required"), False
            record = self.idempotency.get((cid, method, key))
            if record:
                if record[0] != params:
                    return pwc_error("IDEMPOTENCY_CONFLICT", "Key reused with different parameters"), False
                return self.operations[record[1]][1], False
            op_id = "op-" + secrets.token_hex(6)
            op = {"operation_id": op_id, "method": method, "state": "awaiting_approval", "created_at": now(),
                  "updated_at": now(), "expires_at": later(600)}
            self.operations[op_id] = (cid, op)
            self.idempotency[(cid, method, key)] = (params, op_id)
            return op, False
        if method == "operation.get":
            entry = self.operations.get(params.get("operation_id"))
            if not entry or entry[0] != cid:
                return pwc_error("OPERATION_NOT_FOUND", "No such operation"), False
            return entry[1], False
        return pwc_error("UNSUPPORTED_OPERATION", f"{method} has no data in the mock"), False


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", help="endpoint directory (default: the LOCAL §3.2 location)")
    ap.add_argument("--reject-pairing", action="store_true")
    args = ap.parse_args()
    directory = args.dir or endpoint_directory()
    os.makedirs(directory, mode=0o700, exist_ok=True)
    os.chmod(directory, 0o700)
    path = os.path.join(directory, f"{PROVIDER_ID}.sock")
    if os.path.exists(path):
        probe = socket.socket(socket.AF_UNIX)
        try:
            probe.connect(path)
            raise SystemExit(f"another instance is listening on {path}")
        except ConnectionRefusedError:
            os.unlink(path)                                                # stale file (LOCAL §3.2)
        finally:
            probe.close()
    provider = Provider(args.reject_pairing)
    server = await asyncio.start_unix_server(provider.handle, path=path, limit=MAX_FRAME)
    os.chmod(path, 0o600)
    print(f"mock Provider listening on {path}", flush=True)
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
