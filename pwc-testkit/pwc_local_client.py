"""Minimal PWC LOCAL client (PWC-TRANSPORT-LOCAL-001 v0.1.0), macOS and Linux.

Used by the Client guide examples and by local_runner.py. Not a production SDK: it sends one request at a
time and ignores event notifications that arrive while it waits for a response.
"""
import hashlib
import hmac
import json
import os
import secrets
import socket
import stat


def endpoint_directory():
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if os.uname().sysname == "Darwin":
        return os.path.join(os.environ["HOME"], "Library", "Application Support", "PWC", "run")
    if runtime and os.path.isabs(runtime) and os.stat(runtime).st_uid == os.getuid():
        return os.path.join(runtime, "pwc")
    return os.path.join(os.environ["HOME"], ".pwc", "run")


def check_directory(path):
    """LOCAL §3.2: the directory must belong to the user and grant nothing to group or others."""
    st = os.stat(path)
    if st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) & 0o077:
        raise PermissionError(f"unsafe endpoint directory {path}")


def client_proof(secret, server_nonce, client_nonce, client_id):
    msg = b"PWC-LOCAL-1 client\x00" + server_nonce + client_nonce + client_id.encode()
    return hmac.new(secret, msg, hashlib.sha256).digest()


def server_proof(secret, server_nonce, client_nonce, client_id):
    msg = b"PWC-LOCAL-1 provider\x00" + client_nonce + server_nonce + client_id.encode()
    return hmac.new(secret, msg, hashlib.sha256).digest()


class PwcError(Exception):
    def __init__(self, error):
        super().__init__(f"{error.get('code')}: {error.get('message')}")
        self.error = error
        self.code = error.get("code")


class LocalClient:
    def __init__(self, socket_path, timeout=30.0):
        check_directory(os.path.dirname(socket_path))
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(timeout)
        self.sock.connect(socket_path)
        self.reader = self.sock.makefile("rb")
        self.next_id = 1
        self.hello = None

    def close(self):
        self.reader.close()
        self.sock.close()

    def send_raw(self, data):
        self.sock.sendall(data)

    def read_frame(self):
        line = self.reader.readline()
        if not line:
            raise ConnectionError("connection closed by Provider")
        return json.loads(line)

    def call(self, method, params=None):
        """Send one request and return the raw JSON-RPC response object (events are skipped)."""
        rid = self.next_id
        self.next_id += 1
        frame = {"jsonrpc": "2.0", "id": rid, "method": method, "params": params or {}}
        self.send_raw(json.dumps(frame, separators=(",", ":")).encode() + b"\n")
        while True:
            msg = self.read_frame()
            if msg.get("id") == rid:
                return msg

    def request(self, method, params=None):
        """Send one request; return its result or raise PwcError."""
        msg = self.call(method, params)
        if "error" in msg:
            err = msg["error"]
            raise PwcError(err.get("data") or {"code": str(err.get("code")), "message": err.get("message")})
        return msg["result"]

    def say_hello(self):
        self.hello = self.request("local.hello", {"protocols": ["pwc-local/1"]})
        return self.hello

    def pair(self, client_name, delegator=False):
        code = f"{secrets.randbelow(1_000_000):06d}"
        print(f"Pairing code: {code} (compare it with the wallet)")
        params = {"client_name": client_name, "pairing_code": code}
        if delegator:
            params["delegator"] = True
        return self.request("local.pair", params)

    def authenticate(self, client_id, client_secret_hex):
        secret = bytes.fromhex(client_secret_hex)
        server_nonce = bytes.fromhex(self.hello["server_nonce"])
        client_nonce = secrets.token_bytes(32)
        result = self.request("local.authenticate", {
            "client_id": client_id,
            "client_nonce": client_nonce.hex(),
            "client_proof": client_proof(secret, server_nonce, client_nonce, client_id).hex(),
        })
        expected = server_proof(secret, server_nonce, client_nonce, client_id)
        if not hmac.compare_digest(expected, bytes.fromhex(result["server_proof"])):
            self.close()
            raise ConnectionError("Provider failed authentication; connection closed")   # LOCAL §5.4 step 4
        return result
