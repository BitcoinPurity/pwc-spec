"""Example from PWC-CLIENT-GUIDE-001 §4: pair once, reconnect, request permissions, poll the Operation.

Usage: python3 example_local_client.py <socket-path> <credential-file>
"""
import json
import os
import sys
import time
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pwc_local_client import LocalClient, PwcError   # noqa: E402

socket_path, cred_file = sys.argv[1], sys.argv[2]

client = LocalClient(socket_path)
client.say_hello()
if os.path.exists(cred_file):
    cred = json.load(open(cred_file))
    client.authenticate(cred["client_id"], cred["client_secret"])
else:
    paired = client.pair("Example Trader")             # the wallet shows a prompt with the same code
    cred = {"client_id": paired["client_id"], "client_secret": paired["client_secret"]}
    fd = os.open(cred_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)   # readable by this user only
    with os.fdopen(fd, "w") as f:
        json.dump(cred, f)

caps = client.request("pwc.getCapabilities")
print("Provider:", caps["provider"]["name"], caps["pwc_version"])

key = str(uuid.uuid4())                                 # keep it until the Operation is terminal
op = client.request("session.requestPermissions", {"scopes": ["wallets:read", "balances:read"],
                                                    "idempotency_key": key})
for _ in range(3):                                      # poll gently; a real Client waits up to op["expires_at"]
    if op["state"] in ("completed", "rejected", "expired", "canceled", "failed"):
        break
    time.sleep(1)
    op = client.request("operation.get", {"operation_id": op["operation_id"]})
print("permission request:", op["state"])

try:
    client.request("wallet.list")
except PwcError as e:
    print("wallet.list:", e.code)                       # PERMISSION_REQUIRED until the User approves
client.close()
